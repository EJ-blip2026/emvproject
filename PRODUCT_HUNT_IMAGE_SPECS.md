# 🎨 Product Hunt Gallery Images - Design Specifications

## Overview
You need **6 images total**:
- 1 Thumbnail (240x240px)
- 5 Gallery images (1270x760px)

---

## 🖼️ IMAGE 1: THUMBNAIL (240x240px)

### Design Specs:
```
Dimensions: 240 x 240 pixels
Format: PNG (transparent background) or JPG
File size: Under 500KB
```

### Visual:
```
┌─────────────────────┐
│                     │
│      🔐 + 👆        │
│                     │
│   Zero-Knowledge    │
│       Vault         │
│                     │
└─────────────────────┘
```

### Design Instructions:
1. **Background:** Dark gradient (#0b1021 → #0d223c)
2. **Icon:** Large padlock emoji 🔐 or vault icon (centered, top 40%)
3. **Secondary icon:** Fingerprint 👆 or biometric icon (smaller, overlapping bottom-right)
4. **Text:** "Zero-Knowledge Vault" in white, bottom 20%
5. **Font:** Space Grotesk or Inter, 18px, bold

### Create with:
- **Canva:** Use "Logo" template, add icons from library
- **Figma:** Create 240x240 frame, add text + icons
- **Online:** Use https://www.logomakr.com/

---

## 📸 IMAGE 2: HERO SCREENSHOT (1270x760px)
**Focus: Passwordless Login**

### What to capture:
Take a screenshot of: https://emvproject-production.up.railway.app

**Crop to show:**
- Header with "Zero-Knowledge Vault" badge
- Hero text: "Own your secrets end-to-end"
- Status chips including "🔐 Passkey login (biometric)"
- The purple "🔐 Login with Passkey" button (IMPORTANT)

### Annotations to add:
```
┌────────────────────────────────────┐
│  [Screenshot of landing page]     │
│                                    │
│  Arrow pointing to passkey button →│
│  "1-tap biometric login"           │
│                                    │
│  Highlight box around status chip  │
│  "Phishing-proof authentication"   │
└────────────────────────────────────┘
```

### Design Instructions:
1. Take screenshot in Chrome (F12 → responsive mode → 1400px wide)
2. Add yellow arrow pointing to "Login with Passkey" button
3. Add text overlay: **"1-tap biometric login"** (white text, yellow background)
4. Optional: Add green box around "🔐 Passkey login" chip
5. Optional: Add text at bottom: **"No passwords. Ever."** (large, bold)

### Tools:
- **Screenshot:** Browser DevTools responsive mode
- **Annotations:** Canva (upload screenshot, add shapes/text)
- **Alternative:** Snagit, Skitch, or Preview (Mac) markup tools

---

## 🔒 IMAGE 3: ZERO-KNOWLEDGE DIAGRAM (1270x760px)
**Focus: Security Architecture**

### Visual Layout:
```
┌─────────────────────────────────────────────────────────┐
│                                                         │
│   YOUR DEVICE              OUR SERVER                   │
│  ┌──────────┐           ┌──────────────┐              │
│  │ 📄 Files │           │ 🔒 Encrypted │              │
│  │ Password │           │    Bytes     │              │
│  │  (plain) │           │   (gibberish)│              │
│  └──────────┘           └──────────────┘              │
│       ↓                                                 │
│   🔐 Encrypt                                            │
│   (XChaCha20)                                           │
│       ↓                                                 │
│  Encrypted ──upload──→  ✅ Stored                      │
│                                                         │
│  ❌ We CANNOT decrypt your data                        │
│  ✅ Only YOU have the key                              │
│                                                         │
└─────────────────────────────────────────────────────────┘
```

### Design Instructions:

**Left Side (Your Device):**
1. Icon: Laptop/phone 💻
2. Text: "Your Device"
3. Box with: "Plain text files, passwords, notes"
4. Arrow down with lock icon 🔐
5. Text: "Encrypted with XChaCha20-Poly1305"

**Arrow (Middle):**
1. Large arrow →
2. Label: "Upload encrypted bytes only"

**Right Side (Our Server):**
1. Icon: Server/cloud ☁️
2. Text: "Our Server"
3. Box with: Random gibberish characters (k3jf92jf8j2f...)
4. Red X: "❌ We can't read this"

**Bottom Banner:**
1. Green background
2. Text: "✅ True Zero-Knowledge: We don't have your encryption key"

### Colors:
- Background: Dark (#0b1021)
- Boxes: Light with border (#1f2a44)
- Text: White (#e2e8f0)
- Accent: Yellow (#f6c452) for arrows
- Success: Green (#4ade80)

### Create with:
- **Canva:** Search "infographic" template, customize
- **Figma:** Use shapes + text + arrows
- **Excalidraw:** Quick sketchy diagrams

---

## 🎯 IMAGE 4: PASSKEY ENROLLMENT FLOW (1270x760px)
**Focus: Easy Setup**

### Option A: Screenshot Flow (3 panels)
```
┌─────────────┬─────────────┬─────────────┐
│   STEP 1    │   STEP 2    │   STEP 3    │
│             │             │             │
│ Click       │ Touch ID    │ ✅ Done     │
│ "Enroll     │ prompt      │             │
│  Passkey"   │ appears     │ "Enrolled!" │
│             │             │             │
└─────────────┴─────────────┴─────────────┘
```

**Screenshots to take:**
1. Passkey management tab with "Enroll New Passkey" button
2. Browser's Touch ID/Face ID prompt (if possible)
3. Success message "✅ Passkey enrolled successfully!"

**Annotations:**
- Number each step: 1, 2, 3
- Add caption: **"Enroll in 5 seconds"**
- Add time indicator: ⏱️ "< 60 seconds"

### Option B: Simple Graphic

**Layout:**
```
┌──────────────────────────────────────────┐
│                                          │
│     "3 Steps to Passwordless Login"     │
│                                          │
│  1️⃣  Click "Enroll Passkey"             │
│      [Button screenshot]                 │
│                                          │
│  2️⃣  Scan fingerprint / Face ID         │
│      [Fingerprint icon 👆]              │
│                                          │
│  3️⃣  Done! Login with 1 tap            │
│      [Checkmark ✅]                     │
│                                          │
│   "No passwords to remember. Ever."     │
│                                          │
└──────────────────────────────────────────┘
```

### Create with:
- **Canva:** 3-column layout
- **Screenshot sequence:** Use macOS Screenshot (Cmd+Shift+5) or Windows Snipping Tool

---

## 📂 IMAGE 5: VAULT INTERFACE (1270x760px)
**Focus: Clean UI & Features**

### What to capture:
1. Login to your vault (register test account)
2. Create 2-3 demo vaults with names like:
   - "Personal Passwords"
   - "Work Documents"
   - "Financial Records"
3. Take full-page screenshot showing vault list

### Annotations to add:
```
Label key features:
→ "Military-grade encryption" (pointing to vault list)
→ "Organize by category" (pointing to vault names)
→ "Audit logs track everything" (pointing to tabs)
→ "Cloud import ready" (pointing to cloud section)
```

### Design Instructions:
1. Take clean screenshot of vault listing page
2. Add 3-4 callout boxes with arrows
3. Optional: Blur/redact any test data
4. Add header: **"Secure. Organized. Auditable."**

---

## 🌟 IMAGE 6: FEATURES OVERVIEW (1270x760px)
**Focus: Value Proposition**

### Layout (Grid style):
```
┌──────────────────────────────────────────────────┐
│                                                  │
│      "Why Zero-Knowledge Vault?"                │
│                                                  │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐     │
│  │ 🔐       │  │ ⚡       │  │ ☁️       │     │
│  │ Passkey  │  │ Zero-    │  │ Cloud    │     │
│  │ Login    │  │ Knowledge│  │ Import   │     │
│  │          │  │          │  │          │     │
│  │ 1-tap    │  │ We can't │  │ Google   │     │
│  │ biometric│  │ read your│  │ Drive    │     │
│  │          │  │ data     │  │ OneDrive │     │
│  └──────────┘  └──────────┘  └──────────┘     │
│                                                  │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐     │
│  │ 📊       │  │ 🏢       │  │ 🔓       │     │
│  │ Audit    │  │ Enterprise│ │ Open     │     │
│  │ Logs     │  │ Ready    │  │ Source   │     │
│  │          │  │          │  │          │     │
│  │ Complete │  │ HIPAA    │  │ Auditable│     │
│  │ activity │  │ SOC2     │  │ on GitHub│     │
│  │ tracking │  │ compliant│  │          │     │
│  └──────────┘  └──────────┘  └──────────┘     │
│                                                  │
└──────────────────────────────────────────────────┘
```

### Design Instructions:

**Header:**
- Text: "Why Zero-Knowledge Vault?" 
- Font: 48px, bold, white
- Centered

**Grid (2 rows × 3 columns):**

Each card:
- Background: Dark with subtle border
- Icon: 48px emoji or icon (top)
- Title: 24px, bold (e.g., "Passkey Login")
- Description: 16px, 2-3 words (e.g., "1-tap biometric")

**Features to highlight:**
1. 🔐 **Passkey Login** - "1-tap biometric"
2. ⚡ **Zero-Knowledge** - "We can't read your data"
3. ☁️ **Cloud Import** - "Google Drive, OneDrive"
4. 📊 **Audit Logs** - "Complete activity tracking"
5. 🏢 **Enterprise Ready** - "HIPAA, SOC2 compliant"
6. 🔓 **Open Source** - "Auditable on GitHub"

### Create with:
- **Canva:** Search "feature grid" template
- **Figma:** Use auto-layout for grid

---

## 🛠️ TOOLS REFERENCE

### Free Design Tools:
1. **Canva** (easiest): https://canva.com
   - Search "Product Hunt" templates
   - Drag & drop interface
   - Free icons and fonts

2. **Figma** (professional): https://figma.com
   - More control
   - Web-based, no install
   - Steeper learning curve

3. **Excalidraw** (quick diagrams): https://excalidraw.com
   - Perfect for architecture diagrams
   - Sketchy style
   - Very fast

### Screenshot Tools:
- **Mac:** Cmd + Shift + 5 (built-in)
- **Windows:** Snipping Tool or Snip & Sketch
- **Browser:** DevTools responsive mode (1400px width)
- **Full page:** Browser extensions like "GoFullPage"

### Annotation Tools:
- **Mac:** Preview (built-in markup)
- **Windows:** Paint 3D or Snip & Sketch
- **Cross-platform:** Canva (upload → add shapes/text)

---

## ✅ CHECKLIST

Before submitting to Product Hunt:

- [ ] Thumbnail created (240x240px)
- [ ] Image 1: Hero screenshot with annotations
- [ ] Image 2: Zero-knowledge diagram
- [ ] Image 3: Passkey enrollment flow
- [ ] Image 4: Vault interface screenshot
- [ ] Image 5: Features grid
- [ ] All images exported as PNG or JPG
- [ ] All images under 2MB each
- [ ] All text is readable at 1270px width
- [ ] Colors match brand (dark theme)
- [ ] No typos in annotations

---

## 📦 QUICK START

**Don't want to design from scratch?**

### Option 1: Screenshots Only (10 minutes)
1. Take 5 screenshots of your app
2. Upload to Canva
3. Add simple text overlays with their text tool
4. Export as PNG

### Option 2: Use Templates (30 minutes)
1. Go to Canva
2. Search "Product Hunt gallery"
3. Pick a template
4. Replace text/images with yours
5. Export

### Option 3: Hire on Fiverr ($10-30, 24 hours)
1. Go to Fiverr.com
2. Search "Product Hunt images"
3. Send designer this document + screenshots
4. Get polished images in 1 day

---

## 🎯 PRIORITIES

**If you only have time for 3 images:**

1. ✅ **Hero screenshot** (most important - shows the product)
2. ✅ **Zero-knowledge diagram** (unique selling point)
3. ✅ **Features grid** (quick value overview)

**Skip if rushed:**
- Passkey enrollment flow (can describe in text)
- Vault interface (similar to hero)

---

## 📤 EXPORT SETTINGS

When exporting from Canva/Figma:

**Thumbnail:**
- Format: PNG
- Size: 240 x 240 px
- Quality: High

**Gallery Images:**
- Format: PNG (for transparency) or JPG (smaller file)
- Size: 1270 x 760 px
- Quality: 90% (good balance)

---

**Ready to create?** Start with the hero screenshot (easiest) and work your way through. Let me know if you need help with any specific image!
