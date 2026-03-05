# 🔐 What Are Passkeys? Quick Explainer

## The Problem with Passwords
- 💀 **81%** of breaches involve weak/reused passwords
- 😵 Users have to remember 100+ passwords
- 🎣 Phishing still works (convince user to type password)
- 🔓 Passwords stored in databases get hacked

## The Solution: Passkeys
A **passkey** is a cryptographic credential that:
- Lives on YOUR device (not our server)
- Uses biometrics (fingerprint, face) to unlock
- Impossible to phish (origin-bound)
- Works offline, syncs securely online

## How It Works

```
Traditional Password:
User types → (network) → Server checks database → ✅ or ❌

Passkey (WebAuthn):
Device + Biometric → Private key signs challenge → Public key verified → ✅
(Hacker can't impersonate you - they don't have your private key)
```

## Key Advantages

| Feature | Password | Passkey |
|---------|----------|---------|
| Phishable? | ✅ Yes (😞) | ❌ No (😊) |
| Reusable? | ✅ Yes (😞) | ❌ No (😊) |
| Memorable? | ✅ Yes | ❌ No (but device remembers) |
| Fast? | ❌ Slow | ✅ 1 tap |
| Hacked database? | ❌ Fails | ✅ Safe (keys on device) |

## Our Implementation

We use **FIDO2/WebAuthn** standard:
- Supported by Chrome, Safari, Firefox, Edge
- Works on phone, laptop, security keys
- Military-grade cryptography (ECDSA P-256)
- No passwords ever stored

## What We Never Get
- ❌ Your fingerprint
- ❌ Your face scan
- ❌ Your private key
- ❌ Your password

Your biometric unlocks your device. We only see the signature.

## Try It Now

1. Sign up at [vault.app](https://your-domain)
2. Go to "Passkeys" tab
3. Click "Enroll New Passkey"
4. Use fingerprint/face ID
5. Next time, login with just a tap!

---

**Ready for passwordless? Welcome to the future.** 🚀
