# Deployment Checklist - Zero-Knowledge Vault to Railway

## 📦 Build Status
- ✅ Release binary built: `11MB` (optimized)
- ✅ Migrations ready: `0001-0004` (schema complete)
- ✅ Frontend compiled: `1279` lines (WebAuthn + UI complete)
- ✅ Backend API: `1662` lines (all endpoints implemented)

## 🚀 Quick Deploy to Railway

### Step 1: Commit Changes
```bash
cd /workspaces/emvproject
git add -A
git commit -m "feat: Complete WebAuthn/Passkey support, all features ready for deployment"
git push origin main
```

### Step 2: Create Railway Project
1. Go to [railway.app](https://railway.app)
2. Click **New Project** → **Deploy from GitHub**
3. Select `EJ-blip2026/emvproject`
4. Authorize if needed

### Step 3: Configure Database
1. In Railway dashboard, click **Add Service** → **Database** → **PostgreSQL**
2. Railway automatically creates and injects `DATABASE_URL`

### Step 4: Set Environment Variables
In Railway Project Settings → Variables:
```
ADMIN_USERNAME=admin
ADMIN_PASSWORD=YourSecurePasswordHere123!
PORT=3000
```

**Optional:**
```
ADMIN_TOKEN=your_32_char_token_here
```

### Step 5: Deploy
- Railway automatically builds from `Cargo.toml`
- Runs migrations on startup
- Service starts on PORT 3000

## ✅ Verify Deployment

### 1. Health Check
```bash
curl https://your-railway-domain.railway.app/health
# Expected: {"status":"ok","service":"vault-api"}
```

### 2. Create Admin Account
```bash
curl -X POST https://your-railway-domain.railway.app/auth/login \
  -H "Content-Type: application/json" \
  -d '{
    "username": "admin",
    "password": "YourSecurePasswordHere123!"
  }'
```

### 3. Test Vault Creation
```bash
curl -X POST https://your-railway-domain.railway.app/vaults/create \
  -H "Authorization: Bearer YOUR_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{"name":"Test Vault","description":"Testing deployment"}'
```

### 4. Check Usage Panel
```bash
curl https://your-railway-domain.railway.app/account/usage \
  -H "Authorization: Bearer YOUR_TOKEN"
```

## 🔐 Security Checklist

- [ ] Strong `ADMIN_PASSWORD` (32+ chars, mixed case, numbers, symbols)
- [ ] PostgreSQL is external (not SQLite)
- [ ] HTTPS enabled (Railway auto-provides)
- [ ] Audit logs enabled (check `/account/audit-logs`)
- [ ] Health check monitoring (set up uptime check)

## 📊 Features Deployed

### Authentication (✅ Complete)
- ✅ Register with enterprise tier default
- ✅ Login with Argon2id password verification
- ✅ **NEW**: WebAuthn Passkey support
  - Enroll passkeys (biometric/security key)
  - Login with passkey
  - Audit logging for passkey events

### Vault Features (✅ Complete)
- ✅ Encrypted notes with XChaCha20-Poly1305
- ✅ Encrypted file upload/download
- ✅ Multiple vaults per user
- ✅ Cloud storage import (Google Drive, OneDrive)
- ✅ Storage quota enforcement per tier

### Subscription Tiers (✅ Complete)
- ✅ **Starter**: 5GB (default)
- ✅ **Pro**: 100GB ($9.99/mo)
- ✅ **Enterprise**: 1TB ($49.99/mo)
- ✅ Upgrade endpoint
- ✅ Usage panel with 90% warning

### Security (✅ Complete)
- ✅ Argon2id hashing (64MB, 3 iterations)
- ✅ XChaCha20-Poly1305 encryption
- ✅ Audit logging (login, create note, upload file, passkey events)
- ✅ Session tokens in-memory
- ✅ Secret redaction in logs

### Admin Features (✅ Complete)
- ✅ ADMIN_USERNAME/ADMIN_PASSWORD auto-seed
- ✅ Credentials sync on restart
- ✅ Audit log viewing (`/account/audit-logs`)

## 🎯 Post-Deployment Tasks

### 1. Set Up Monitoring
- Configure Railway uptime notifications
- Set health check every 5 minutes
- Alert on failures

### 2. Test Frontend
- Visit `https://your-domain/`
- Test login/register flow
- Test WebAuthn enrollment
- Test vault creation and notes

### 3. Load Testing (Optional)
```bash
# Quick load test with Apache Bench
ab -n 100 -c 10 https://your-domain/health
```

### 4. Review Logs
- Railway dashboard → Logs tab
- Search for any errors
- Check migration output

## 📱 Frontend Test Checklist

### Authentication Tab
- [ ] Register new user
- [ ] Login with password
- [ ] Login with passkey
- [ ] Logout

### Vault Tab
- [ ] Create vault
- [ ] Create encrypted note
- [ ] Upload encrypted file
- [ ] View storage usage and 90% warning

### Passkey Tab
- [ ] Enroll new passkey
- [ ] See enrolled passkins list
- [ ] Use passkey to login (on supported device)

### Admin
- [ ] Login as admin
- [ ] View audit logs at `/account/audit-logs`
- [ ] See login events, note creation, file uploads

## 🚨 Troubleshooting

### "Database connection failed"
- Railway PostgreSQL service status: check dashboard
- `DATABASE_URL` format: should auto-inject
- Migrations: run manually if needed

### "502 Bad Gateway"
- Check logs in Railway dashboard
- Verify all environment variables set
- Ensure PORT is exposed (should be 3000)

### "Migrations failed"
- SSH into Railway service
- Run: `DATABASE_URL=... ./target/release/emvproject`
- Check migration file syntax

### WebAuthn not working
- Browser must support WebAuthn API (Chrome/Safari/Edge)
- Must be HTTPS (Railway auto-provides)
- Domain must match RP ID (auto-set to hostname)

## 📞 Support

For issues, check:
1. Railway logs: Dashboard → Logs tab
2. Local test: `DATABASE_URL=sqlite:// cargo run`
3. Health endpoint: `/health` should return 200
4. Audit logs: `/account/audit-logs` endpoint

---

**Deployment Date**: March 5, 2026
**Version**: 1.0.0
**Status**: Ready for Production ✅
