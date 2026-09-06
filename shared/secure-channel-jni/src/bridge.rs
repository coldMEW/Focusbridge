#[cfg(test)]
mod tests {
    use crate::registry::Registry;
    use focusbridge_secure_channel::{Identity, Session, MAX_RECORD};

    #[test]
    fn phone_desktop_confirmation_fragmentation_and_replay() {
        let phone = Identity::from_private([7; 32]).unwrap();
        let desktop = Identity::from_private([8; 32]).unwrap();
        let mut peer = Session::desktop(&desktop, &[9; 32], [1; 16], phone.public_key()).unwrap();
        let mut registry = Registry::new();
        let handle = registry
            .insert_with(|| {
                Ok(Session::phone(
                    &phone,
                    &[9; 32],
                    [1; 16],
                    desktop.public_key(),
                )?)
            })
            .unwrap();
        registry
            .with_session(handle, |session| {
                peer.read_handshake(&session.write_handshake()?)?;
                session.read_handshake(&peer.write_handshake()?)?;
                peer.read_handshake(&session.write_handshake()?)?;
                assert!(!session.is_ready());
                session.read_confirmation(&peer.write_confirmation()?)?;
                assert!(!session.is_ready());
                peer.read_confirmation(&session.write_confirmation()?)?;
                assert!(session.is_ready() && peer.is_ready());
                let plaintext = vec![42; MAX_RECORD];
                let frames = session.seal_record(&plaintext)?;
                assert!(frames.len() > 1);
                let mut received = None;
                for frame in frames {
                    received = peer.open_frame(&frame)?;
                }
                assert_eq!(received.unwrap().as_slice(), plaintext);
                let frames = peer.seal_record(&plaintext)?;
                for (index, frame) in frames.iter().enumerate() {
                    let opened = session.open_frame(frame)?;
                    if index + 1 == frames.len() {
                        assert_eq!(opened.unwrap().as_slice(), plaintext);
                    } else {
                        assert!(opened.is_none());
                    }
                }
                assert!(session.open_frame(&frames[0]).is_err());
                assert!(session.is_closed());
                Ok(())
            })
            .unwrap();
        assert!(registry
            .with_session(handle, |s| Ok(s.check_alive()?))
            .is_err());
        assert!(registry.with_session(handle, |_| Ok(())).is_err());
    }

    #[test]
    fn input_limits_reject_empty_and_oversized() {
        assert!(super::validate_length(0, 1, 256).is_err());
        assert!(super::validate_length(257, 1, 256).is_err());
        assert!(super::validate_length(256, 1, 256).is_ok());
        assert!(super::validate_length(31, 32, 32).is_err());
    }
}
use crate::{registry::Registry, Error, Result};
use focusbridge_secure_channel::{Identity, Session, MAX_FRAME, MAX_RECORD};
use jni::{
    objects::{JByteArray, JObject},
    sys::{jbyteArray, jlong, jobjectArray},
    JNIEnv,
};
use std::sync::{Mutex, OnceLock};
use zeroize::Zeroizing;

static REGISTRY: OnceLock<Mutex<Registry>> = OnceLock::new();

fn registry() -> &'static Mutex<Registry> {
    REGISTRY.get_or_init(|| Mutex::new(Registry::new()))
}

fn validate_length(length: usize, min: usize, max: usize) -> Result<()> {
    if (min..=max).contains(&length) {
        Ok(())
    } else {
        Err(Error::InvalidInput)
    }
}

fn bytes(env: &JNIEnv, array: &JByteArray, min: usize, max: usize) -> Result<Zeroizing<Vec<u8>>> {
    if array.is_null() {
        return Err(Error::InvalidInput);
    }
    let length = env.get_array_length(array).map_err(|_| Error::Jni)? as usize;
    validate_length(length, min, max)?;
    Ok(Zeroizing::new(
        env.convert_byte_array(array).map_err(|_| Error::Jni)?,
    ))
}

fn output(env: &JNIEnv, value: &[u8]) -> Result<jbyteArray> {
    Ok(env
        .byte_array_from_slice(value)
        .map_err(|_| Error::Jni)?
        .into_raw())
}

// Keep marshalling under the lock: output failure after nonce advancement must retire the handle.
fn boundary<T: Default>(
    env: &mut JNIEnv,
    handle: Option<i64>,
    op: impl FnOnce(&mut JNIEnv, &mut Registry) -> Result<T>,
) -> T {
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut guard = registry().lock().map_err(|_| Error::Internal)?;
        op(env, &mut guard)
    }))
    .unwrap_or(Err(Error::Internal));
    match result {
        Ok(value) => value,
        Err(_) => {
            match registry().lock() {
                Ok(mut guard) => {
                    if let Some(id) = handle {
                        guard.close(id);
                    }
                }
                Err(poison) => {
                    poison.into_inner().close_all();
                }
            }
            // Preserve an existing VM exception (notably OOM); never return success on failure.
            if !env.exception_check().unwrap_or(true) {
                let _ = env.throw_new(
                    "java/lang/IllegalStateException",
                    "Secure channel failed; discard session",
                );
            }
            T::default()
        }
    }
}

#[no_mangle]
pub extern "system" fn Java_com_focusbridge_android_sync_secure_NativeSecureChannel_createPhone(
    mut env: JNIEnv,
    _: JObject,
    private: JByteArray,
    psk: JByteArray,
    pair: JByteArray,
    desktop: JByteArray,
) -> jlong {
    boundary(&mut env, None, |env, registry| {
        // Consume caller-owned secrets even when construction fails. Wipe before any other parsing.
        let private_copy = bytes(env, &private, 32, 32);
        let psk_copy = bytes(env, &psk, 32, 32);
        let wipe_private = wipe(env, &private);
        let wipe_psk = wipe(env, &psk);
        let private = private_copy?;
        let psk = psk_copy?;
        wipe_private?;
        wipe_psk?;
        let pair = bytes(env, &pair, 16, 16)?;
        let desktop = bytes(env, &desktop, 32, 32)?;
        registry.insert_with(|| {
            let identity = Identity::from_private(
                private
                    .as_slice()
                    .try_into()
                    .map_err(|_| Error::InvalidInput)?,
            )?;
            Ok(Session::phone(
                &identity,
                psk.as_slice().try_into().map_err(|_| Error::InvalidInput)?,
                pair.as_slice()
                    .try_into()
                    .map_err(|_| Error::InvalidInput)?,
                desktop
                    .as_slice()
                    .try_into()
                    .map_err(|_| Error::InvalidInput)?,
            )?)
        })
    })
}

fn wipe(env: &JNIEnv, array: &JByteArray) -> Result<()> {
    if array.is_null() {
        return Err(Error::InvalidInput);
    }
    let length = env.get_array_length(array).map_err(|_| Error::Jni)?;
    // Fixed scratch space, including for malformed oversized secret arrays.
    let zeros = [0i8; 256];
    let mut offset = 0;
    while offset < length {
        let count = (length - offset).min(256);
        env.set_byte_array_region(array, offset, &zeros[..count as usize])
            .map_err(|_| Error::Jni)?;
        offset += count;
    }
    Ok(())
}

macro_rules! write_export {
    ($name:ident, $method:ident) => {
        #[no_mangle]
        pub extern "system" fn $name(mut env: JNIEnv, _: JObject, handle: jlong) -> jbyteArray {
            boundary(&mut env, Some(handle), |env, registry| {
                registry.with_session(handle, |s| output(env, &s.$method()?))
            })
        }
    };
}
write_export!(
    Java_com_focusbridge_android_sync_secure_NativeSecureChannel_writeHandshake,
    write_handshake
);
write_export!(
    Java_com_focusbridge_android_sync_secure_NativeSecureChannel_writeConfirmation,
    write_confirmation
);

macro_rules! read_export {
    ($name:ident, $method:ident, $max:expr) => {
        #[no_mangle]
        pub extern "system" fn $name(
            mut env: JNIEnv,
            _: JObject,
            handle: jlong,
            frame: JByteArray,
        ) {
            boundary(&mut env, Some(handle), |env, registry| {
                registry.with_session(handle, |s| {
                    let frame = bytes(env, &frame, 1, $max)?;
                    Ok(s.$method(&frame)?)
                })
            })
        }
    };
}
read_export!(
    Java_com_focusbridge_android_sync_secure_NativeSecureChannel_readHandshake,
    read_handshake,
    256
);
read_export!(
    Java_com_focusbridge_android_sync_secure_NativeSecureChannel_readConfirmation,
    read_confirmation,
    256
);

#[no_mangle]
pub extern "system" fn Java_com_focusbridge_android_sync_secure_NativeSecureChannel_seal(
    mut env: JNIEnv,
    _: JObject,
    handle: jlong,
    plaintext: JByteArray,
) -> jobjectArray {
    boundary(&mut env, Some(handle), |env, registry| {
        registry.with_session(handle, |s| {
            let plaintext = bytes(env, &plaintext, 1, MAX_RECORD)?;
            let frames = s.seal_record(&plaintext)?;
            let result = env
                .new_object_array(frames.len() as i32, "[B", JObject::null())
                .map_err(|_| Error::Jni)?;
            for (index, frame) in frames.iter().enumerate() {
                let array =
                    env.auto_local(env.byte_array_from_slice(frame).map_err(|_| Error::Jni)?);
                env.set_object_array_element(&result, index as i32, &array)
                    .map_err(|_| Error::Jni)?;
            }
            Ok(result.into_raw())
        })
    })
}

#[no_mangle]
pub extern "system" fn Java_com_focusbridge_android_sync_secure_NativeSecureChannel_open(
    mut env: JNIEnv,
    _: JObject,
    handle: jlong,
    frame: JByteArray,
) -> jbyteArray {
    boundary(&mut env, Some(handle), |env, registry| {
        registry.with_session(handle, |s| {
            let frame = bytes(env, &frame, 16, MAX_FRAME)?;
            match s.open_frame(&frame)? {
                Some(plaintext) => output(env, &plaintext),
                None => Ok(std::ptr::null_mut()),
            }
        })
    })
}

#[no_mangle]
pub extern "system" fn Java_com_focusbridge_android_sync_secure_NativeSecureChannel_isReady(
    mut env: JNIEnv,
    _: JObject,
    handle: jlong,
) -> u8 {
    boundary(&mut env, Some(handle), |_, registry| {
        registry.with_session(handle, |s| {
            s.check_alive()?;
            Ok(u8::from(s.is_ready()))
        })
    })
}

#[no_mangle]
pub extern "system" fn Java_com_focusbridge_android_sync_secure_NativeSecureChannel_close(
    mut env: JNIEnv,
    _: JObject,
    handle: jlong,
) {
    boundary(&mut env, Some(handle), |_, registry| {
        registry.close(handle);
        Ok(())
    })
}
