# Code signing

Release builds of `vizz.exe` are signed in GitHub Actions (`.github/workflows/release.yml`). The
workflow picks the first configured method and skips signing (with a warning) if none is set.

## Option A: SignPath Foundation (free for open source)

1. Apply at <https://signpath.org> with the repository URL, licence (GPL-3.0) and releases URL.
2. After approval, create a project in SignPath and upload `.signpath/artifact-configuration.xml` as
   its artifact configuration. Link the GitHub repository as a trusted build system.
3. In GitHub → Settings → Secrets and variables → Actions:
   - Variables: `SIGNPATH_ORGANIZATION_ID`, `SIGNPATH_PROJECT_SLUG`, `SIGNPATH_POLICY_SLUG`
     (e.g. `release-signing`)
   - Secret: `SIGNPATH_API_TOKEN`
4. Push a `v*` tag. The job waits for the signing request; approve it in SignPath if the policy asks.

### Code signing policy

Free code signing provided by [SignPath.io](https://signpath.io), certificate by
[SignPath Foundation](https://signpath.org). Only binaries built by this repository's public GitHub
Actions workflow from tagged commits are submitted. Committers and reviewers: @lahirukas. Approver:
@lahirukas. Vizz does not collect data or send anything over the network.

## Option B: certificate file (.pfx)

Publicly trusted code-signing keys must live on hardware or a cloud HSM (CA/B Forum rule since June
2023), so a `.pfx` is in practice a self-signed certificate. It proves the exe was not modified after
the build but does not remove SmartScreen warnings. Create one in PowerShell:

```powershell
$cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject "CN=Vizz (Lahiru Kas)" `
  -KeyAlgorithm RSA -KeyLength 3072 -HashAlgorithm SHA256 `
  -CertStoreLocation Cert:\CurrentUser\My -NotAfter (Get-Date).AddYears(3)
$pass = Read-Host "PFX password" -AsSecureString
Export-PfxCertificate -Cert $cert -FilePath "$HOME\vizz-signing.pfx" -Password $pass
Export-Certificate -Cert $cert -FilePath "$HOME\vizz-signing.cer"
```

Add it to the repository:

- Secret `WINDOWS_CERT_PFX`: the file as base64
  (`[Convert]::ToBase64String([IO.File]::ReadAllBytes("cert.pfx"))`)
- Secret `WINDOWS_CERT_PASSWORD`: its password

The exe is signed with SHA-256 and an RFC 3161 timestamp, then verified before packaging.

Azure Artifact Signing (formerly Trusted Signing) is currently limited to organisations in the US and
Canada, so it is not wired up.
