import { initializeApp } from "firebase/app";
import {
  createUserWithEmailAndPassword,
  getAuth,
  onAuthStateChanged,
  sendEmailVerification,
  sendPasswordResetEmail,
  signInWithEmailAndPassword,
  type User,
} from "firebase/auth";

const firebaseConfig = {
  apiKey: import.meta.env.VITE_FIREBASE_API_KEY ?? "AIzaSyAh06CqVY5qvw5JN0XDvG22gFgqns1jmFQ",
  authDomain: import.meta.env.VITE_FIREBASE_AUTH_DOMAIN ?? "foucsbridge.firebaseapp.com",
  projectId: import.meta.env.VITE_FIREBASE_PROJECT_ID ?? "foucsbridge",
  storageBucket: import.meta.env.VITE_FIREBASE_STORAGE_BUCKET ?? "foucsbridge.firebasestorage.app",
  messagingSenderId: import.meta.env.VITE_FIREBASE_MESSAGING_SENDER_ID ?? "51760085622",
  appId: import.meta.env.VITE_FIREBASE_APP_ID ?? "1:51760085622:web:b509ef61a8dc68dc007d57",
  measurementId: import.meta.env.VITE_FIREBASE_MEASUREMENT_ID ?? "G-3ZZK3G0E7K",
};

const app = initializeApp(firebaseConfig);
const auth = getAuth(app);

export interface FirebaseAuthResult {
  email: string;
  uid: string;
  idToken: string;
}

export async function firebaseEmailSignUp(
  email: string,
  password: string,
): Promise<FirebaseAuthResult> {
  const credential = await createUserWithEmailAndPassword(auth, email.trim(), password);
  return toResult(credential.user);
}

export async function firebaseEmailSignIn(
  email: string,
  password: string,
): Promise<FirebaseAuthResult> {
  const credential = await signInWithEmailAndPassword(auth, email.trim(), password);
  return toResult(credential.user);
}

export async function firebaseCurrentUser(): Promise<FirebaseAuthResult | null> {
  if (auth.currentUser) return toResult(auth.currentUser);
  return new Promise((resolve) => {
    const unsubscribe = onAuthStateChanged(auth, async (user) => {
      unsubscribe();
      resolve(user ? await toResult(user) : null);
    });
  });
}

/**
 * A freshly minted ID token for one relay request.
 *
 * The relay only provisions pairs for verified accounts, so the caller must be
 * able to tell the difference between "not signed in" and "email not verified".
 */
export async function firebaseRelayToken(): Promise<
  { ok: true; idToken: string } | { ok: false; reason: "signed-out" | "unverified" }
> {
  const user = auth.currentUser;
  if (!user) return { ok: false, reason: "signed-out" };
  // Refresh first: verification completed in a browser is not reflected in a
  // token minted before the user clicked the link.
  await user.reload();
  if (!user.emailVerified) return { ok: false, reason: "unverified" };
  return { ok: true, idToken: await user.getIdToken(true) };
}

export async function firebaseSendVerificationEmail(): Promise<void> {
  const user = auth.currentUser;
  if (!user) throw new Error("Sign in first to verify this email address.");
  await sendEmailVerification(user);
}

export async function firebaseSendPasswordReset(email: string): Promise<void> {
  await sendPasswordResetEmail(auth, email.trim());
}

async function toResult(user: User): Promise<FirebaseAuthResult> {
  return {
    email: user.email ?? "",
    uid: user.uid,
    idToken: await user.getIdToken(),
  };
}
