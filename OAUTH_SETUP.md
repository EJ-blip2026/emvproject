# OAuth Setup Guide

## Google Drive OAuth

1. Go to [Google Cloud Console](https://console.cloud.google.com/)
2. Create a new project or select existing
3. Enable the **Google Drive API**
4. Go to **Credentials** → **Create Credentials** → **OAuth 2.0 Client ID**
5. Configure OAuth consent screen (External, add your email)
6. Create OAuth Client ID:
   - Application type: **Web application**
   - Authorized redirect URIs: Add your domain + `/oauth-callback`
     - Example: `https://your-app.up.railway.app/oauth-callback`
     - For local: `http://localhost:3000/oauth-callback`
7. Copy the **Client ID**
8. Set Railway environment variable:
   ```
   GOOGLE_CLIENT_ID=your-client-id-here.apps.googleusercontent.com
   ```

## OneDrive OAuth (Microsoft Graph)

1. Go to [Azure Portal](https://portal.azure.com/)
2. Navigate to **Azure Active Directory** → **App registrations**
3. Click **New registration**
4. Configure:
   - Name: Your app name
   - Supported account types: **Accounts in any organizational directory and personal Microsoft accounts**
   - Redirect URI: **Single-page application (SPA)** → `https://your-app.up.railway.app/oauth-callback`
5. After creation, copy the **Application (client) ID**
6. Go to **API permissions** → **Add a permission** → **Microsoft Graph** → **Delegated permissions**
7. Add permission: `Files.Read.All`
8. Set Railway environment variable:
   ```
   ONEDRIVE_CLIENT_ID=your-application-id-here
   ```

## Frontend Template Replacement

The frontend uses template strings `${GOOGLE_CLIENT_ID}` and `${ONEDRIVE_CLIENT_ID}`. 

### Option 1: Build-time replacement
Add a build script that replaces these with actual env vars before serving.

### Option 2: Runtime config endpoint
Create `/api/config` endpoint that returns:
```json
{
  "google_client_id": "...",
  "onedrive_client_id": "..."
}
```

### Option 3: Direct in HTML (current)
For quick deployment, manually replace in `public/index.html`:
```javascript
const clientId = 'your-actual-client-id-here';
```

## Testing OAuth Flow

1. Set the client IDs (see above)
2. Deploy or run locally
3. Login to your vault
4. Click "📁 Google Drive" or "☁️ OneDrive"
5. OAuth popup opens
6. Approve permissions
7. Popup closes, files load automatically

## Troubleshooting

- **"redirect_uri_mismatch"**: Add exact URL to OAuth console redirect URIs
- **Popup blocked**: Allow popups for your domain
- **Token not received**: Check browser console for postMessage errors
- **CORS errors**: Ensure redirect URI matches exactly (http vs https, trailing slash)

## Security Notes

- Client IDs are public and safe to expose in frontend
- Never expose client secrets in frontend code
- Use implicit flow (token in URL fragment) for SPA apps
- Tokens expire; users must re-authenticate periodically
