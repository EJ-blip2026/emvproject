# Deployment Guide

This guide covers deploying the Haiku API to production using **Railway** or **Render**. Both platforms support Rust, PostgreSQL, and environment variables.

## Prerequisites

- GitHub account (repository must be public or private with access)
- Stripe account (API keys and webhook signing secret)
- Domain (optional, but recommended for webhooks)

## Deployment Option 1: Railway.app (Recommended)

Railway is developer-friendly with automatic deployments from Git.

### Step 1: Create Railway Project

1. Go to [railway.app](https://railway.app)
2. Click **New Project** → **Deploy from GitHub**
3. Select your `emvproject` repository
4. Railway auto-detects the Rust project and builds it

### Step 2: Add PostgreSQL Database

1. In the Railway project dashboard, click **Add Service** → **Database** → **PostgreSQL**
2. Railway automatically creates and links the `DATABASE_URL` environment variable

### Step 3: Set Environment Variables

In the Railway project, go to **Variables** and add:

```
STRIPE_API_KEY=sk_test_your_key_here
STRIPE_WEBHOOK_SECRET=whsec_your_secret_here
ADMIN_TOKEN=your_secure_admin_token_here
RATE_LIMIT=60
RATE_WINDOW_SECS=60
REDIS_URL=redis://...  # Optional, Railway can add Redis service
```

### Step 4: Run Migrations on First Deploy

1. After the build completes, go to the **Deployments** tab
2. Click on the latest deployment → **Logs**
3. The migrations should run automatically on startup (or SSH into the container and run `cargo run -- migrate`)

### Step 5: Update Stripe Webhook URL

1. In Stripe Dashboard → **Webhooks** → **Add Endpoint**
2. Set endpoint URL to: `https://your-railway-domain.railway.app/billing/webhook`
3. Select events: `checkout.session.completed`, `invoice.payment_succeeded`, `invoice.payment_failed`, `customer.subscription.updated`, `customer.subscription.deleted`
4. Copy the **Signing Secret** and add it to Railway variables as `STRIPE_WEBHOOK_SECRET`

### Step 6: Deploy

1. Push changes to GitHub:
   ```bash
   git add .
   git commit -m "Add pricing page and deployment"
   git push origin main
   ```
2. Railway automatically deploys on push

---

## Deployment Option 2: Render.com

Render is a good alternative with free tier support.

### Step 1: Create Render Account

1. Go to [render.com](https://render.com)
2. Sign up with GitHub

### Step 2: Create Web Service

1. Click **New** → **Web Service**
2. Select your GitHub repository
3. Configure:
   - **Name**: `haiku-api`
   - **Environment**: `Rust`
   - **Build Command**: `cargo build --release`
   - **Start Command**: `./target/release/emvproject`
   - **Plan**: Free tier (or paid if you need guaranteed uptime)

### Step 3: Add PostgreSQL Database

1. Click **New** → **PostgreSQL**
2. Name it `haiku-db`
3. Render will provide a `DATABASE_URL` — Render auto-links this to your web service

### Step 4: Set Environment Variables

In the Web Service → **Environment**, add the same variables as Railway:

```
STRIPE_API_KEY=sk_test_your_key_here
STRIPE_WEBHOOK_SECRET=whsec_your_secret_here
ADMIN_TOKEN=your_secure_admin_token_here
RATE_LIMIT=60
RATE_WINDOW_SECS=60
REDIS_URL=redis://...  # Optional
```

### Step 5: Run Migrations

SSH into the service or add a migration script:

```bash
# Option A: Add to start command (runs before server)
./target/release/emvproject migrate && ./target/release/emvproject

# Option B: SSH and run manually
render exec emvproject -- cargo run -- migrate
```

### Step 6: Update Stripe Webhook

Same as Railway — add endpoint pointing to `https://your-render-domain.onrender.com/billing/webhook`

---

## Local Testing Before Deploy

### Test End-to-End with ngrok

1. Start your local server:
   ```bash
   make migrate
   make run
   ```

2. In another terminal, expose it with ngrok:
   ```bash
   ngrok http 3000
   ```

3. Copy the ngrok URL (e.g., `https://abc123.ngrok.io`) and add it to Stripe webhooks for testing

4. Test the flow:
   ```bash
   # Create a checkout session
   curl -X POST http://localhost:3000/billing/create-checkout-session \
     -H "Content-Type: application/json" \
     -d '{"price_id":"price_1234","customer_email":"test@example.com"}'
   
   # Simulate a webhook (use Stripe CLI for real testing)
   curl -X POST http://localhost:3000/billing/webhook \
     -H "Stripe-Signature: t=1234567890,v1=abc123" \
     -d '{"id":"evt_test","type":"checkout.session.completed","data":{"object":{"id":"cs_test","customer_email":"test@example.com"}}}'
   ```

---

## Production Checklist

- [ ] Use strong `ADMIN_TOKEN` (32+ chars)
- [ ] Set up a custom domain with HTTPS
- [ ] Enable Stripe webhook signature verification (done in code)
- [ ] Monitor logs and errors in Railway/Render dashboard
- [ ] Set up uptime monitoring (e.g., UptimeRobot)
- [ ] Test key rotation and subscription lifecycle
- [ ] Add email notifications (SendGrid/Mailgun integration)
- [ ] Scale Redis for distributed rate-limiting (paid tier)
- [ ] Implement analytics dashboard
- [ ] Document API in OpenAPI/Swagger format

---

## Rollback

**Railway**: Go to **Deployments** and click **Rollback** on a previous deployment.

**Render**: Redeploy by connecting to an earlier Git commit or manually restart the service.

---

## Monitoring

Both platforms provide logs and metrics:

- **Railway**: Dashboard shows CPU, memory, network
- **Render**: Logs available in the web service dashboard

For production, consider:
- Prometheus + Grafana for metrics
- Sentry or DataDog for error tracking
- CloudFlare for CDN and DDoS protection

---

## Cost Estimates (as of Jan 2026)

| Platform | Web Service | PostgreSQL | Total/month |
|----------|-----------|-----------|-----------|
| Railway  | $7-25     | $15-30    | $22-55    |
| Render   | Free-$12  | $15+      | $15-27    |

Both offer free tiers for testing. Start with free tier, scale as revenue grows.
