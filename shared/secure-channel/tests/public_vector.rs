//! Published Cacophony vector bundled by Snow v0.10.0:
//! https://github.com/mcginty/snow/blob/v0.10.0/tests/vectors/cacophony.txt
//! Fixed ephemeral keys exist only in this test, never in the Session API.
use focusbridge_secure_channel::PROFILE;

fn bytes(hex: &str) -> Vec<u8> {
    hex::decode(hex).unwrap()
}

#[test]
fn noise_profile_matches_independent_cacophony_bytes_and_handshake_hash() {
    let psk: [u8; 32] = bytes("54686973206973206d7920417573747269616e20706572737065637469766521")
        .try_into()
        .unwrap();
    let init_static = bytes("e61ef9919cde45dd5f82166404bd08e38bceb5dfdfded0a34c8df7ed542214d1");
    let resp_static = bytes("4a3acbfdb163dec651dfa3194dece676d437029c62a408b4c5ea9114246e4893");
    let init_ephemeral = bytes("893e28b9dc6ca8d611ab664754b8ceb7bac5117349a4439a6b0569da977c464a");
    let resp_ephemeral = bytes("bbdb4cdbd309f1a1f2e1456967fe288cadd6f712d65dc7b7793d5e63da6b375b");
    let prologue = bytes("4a6f686e2047616c74");
    let mut init = snow::Builder::new(PROFILE.parse().unwrap())
        .psk(3, &psk)
        .unwrap()
        .prologue(&prologue)
        .unwrap()
        .local_private_key(&init_static)
        .unwrap()
        .fixed_ephemeral_key_for_testing_only(&init_ephemeral)
        .build_initiator()
        .unwrap();
    let mut resp = snow::Builder::new(PROFILE.parse().unwrap())
        .psk(3, &psk)
        .unwrap()
        .prologue(&prologue)
        .unwrap()
        .local_private_key(&resp_static)
        .unwrap()
        .fixed_ephemeral_key_for_testing_only(&resp_ephemeral)
        .build_responder()
        .unwrap();
    let vectors = [
        ("4c756477696720766f6e204d69736573", "ca35def5ae56cec33dc2036731ab14896bc4c75dbb07a61f879f8e3afa4c7944c9f5ff0e8079630cb7e270c20bbf480821b77a384a645c71a2fd9b3db1c16a5f"),
        ("4d757272617920526f746862617264", "95ebc60d2b1fa672c1f46a8aa265ef51bfe38e7ccb39ec5be34069f144808843b123def17f71e6ae8e57e0e1dec5949c5f7415c6f33517398747d821a06dc23ad430aa1fd7381d46195c378a819fd574425462cbb2d4ca339e738a0b7001dc91423fbf55a99af0c6f1df21012ceb2f"),
        ("462e20412e20486179656b", "52187316111b118d4c060364f7b975dc0809b2590779aff2d63113c564f11744493384db7bf32d5ae6686df6ab06d508d2e07caaf1d6afc010b978735fc78900e71ae1d314130d042e729a"),
    ];
    for (index, (payload, expected)) in vectors.iter().enumerate() {
        let (sender, receiver) = if index % 2 == 0 {
            (&mut init, &mut resp)
        } else {
            (&mut resp, &mut init)
        };
        let mut ciphertext = [0; 256];
        let length = sender
            .write_message(&bytes(payload), &mut ciphertext)
            .unwrap();
        assert_eq!(&ciphertext[..length], bytes(expected));
        let mut decoded = [0; 256];
        let count = receiver
            .read_message(&ciphertext[..length], &mut decoded)
            .unwrap();
        assert_eq!(&decoded[..count], bytes(payload));
    }
    let expected_hash = bytes("a477edf6a131bbdb54707f6ea30eab6cd935d9b560f0e5fd1f053a95a99669fb");
    assert_eq!(init.get_handshake_hash(), expected_hash);
    assert_eq!(resp.get_handshake_hash(), expected_hash);
    let mut init = init.into_transport_mode().unwrap();
    let mut resp = resp.into_transport_mode().unwrap();
    for (index, (payload, expected)) in [
        (
            "4361726c204d656e676572",
            "eaedc672d4c21e0e2955758756fb98f194c4e90d5deb5b6cf30b27",
        ),
        (
            "4a65616e2d426170746973746520536179",
            "522d543c5fe799d09a3d9da7ff54d0dc03c8af1dc7751d2ff708339d2290943e98",
        ),
        (
            "457567656e2042f6686d20766f6e2042617765726b",
            "7d4e2c3873eef6a213b04e72f9df60a91666072d3544c5d96c34a09e2329b5030bee796741",
        ),
    ]
    .iter()
    .enumerate()
    {
        let (sender, receiver) = if index % 2 == 0 {
            (&mut resp, &mut init)
        } else {
            (&mut init, &mut resp)
        };
        let mut output = [0; 256];
        let len = sender.write_message(&bytes(payload), &mut output).unwrap();
        assert_eq!(&output[..len], bytes(expected));
        let mut decoded = [0; 256];
        let len = receiver.read_message(&output[..len], &mut decoded).unwrap();
        assert_eq!(&decoded[..len], bytes(payload));
    }
}
