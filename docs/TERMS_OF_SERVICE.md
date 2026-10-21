# Vertex — Terms of Use

**Effective date:** October 1, 2026  
**Maintainer:** rainaku  
**Project:** https://github.com/rainaku/Vertex  
**Applies to:** Official Windows desktop releases and source code repository materials under the Apache License 2.0.

> **Important:** Vertex is an independent, free, and open-source desktop application. It is not a hosted cloud service. These terms describe the conditions governing official distributions and supplementary project materials without amending statutory rights or overriding open-source licenses.

## 1. Scope and assent

These Terms govern your use of the official Vertex desktop application, installers, and project repository. Installing, running, or distributing official builds constitutes assent to these terms where permitted by applicable law. If you disagree with any part of these terms, you may stop using official builds without affecting any independent rights granted to you under the open-source license. Vertex runs entirely offline on your computer and requires no user registration, credentials, or accounts.

## 2. Open-source rights and attribution

Vertex source code is licensed under the Apache License 2.0. You may run, modify, fork, redistribute, and package the software in source or binary form, provided you comply with the conditions of the Apache-2.0 license:

- Keep existing copyright, patent, trademark, and attribution notices.
- Prominently document modifications if you distribute altered versions.
- Include a copy of the Apache-2.0 license with your redistribution.

The name "Vertex" and the identity of maintainer rainaku distinguish official releases from third-party forks. Do not distribute modified or unofficial binaries under representations that they are official builds or endorsed by the maintainer.

## 3. Local conversion and file integrity

Vertex operates locally on your device:

- **Local execution:** Dragging a file into the radial wheel processes that file on your local hardware. Files are never uploaded to remote servers or shared with external parties.
- **Backups:** File encoding and format conversion alter file structures and compression streams. Always keep backups of original files before running batch or destructive conversion operations.
- **External tools:** Audio and video operations depend on local installations of FFmpeg. FFmpeg is an independent open-source project subject to its own licenses (LGPL/GPL). Vertex does not claim ownership of FFmpeg or its dependencies.

## 4. User responsibility and content rights

You retain all rights to the media, images, audio, and video files you process through Vertex:

- You are solely responsible for verifying that your use of source files complies with copyright laws, third-party licenses, and local regulations.
- Vertex is an agnostic local conversion utility. The maintainer does not monitor, evaluate, or index the files you convert.
- Do not use official project channels to distribute malicious software, corrupted payloads, or unauthorized copyrighted materials.

## 5. Updates and signature verification

Vertex includes an optional background update checker that queries official GitHub Releases (`https://github.com/rainaku/Vertex/releases`):

- Official release packages provide cryptographic signatures (Minisign) and SHA-256 checksums.
- Automatic or manual update installations replace local executable files with newly downloaded binaries.
- You can disable or bypass update checks by blocking network requests for the application in your firewall or running from source.

## 6. Disclaimer of warranties

Vertex is provided **"AS IS"** and **"AS AVAILABLE"**, without warranty of any kind, express or implied, including but not limited to the warranties of merchantability, fitness for a particular purpose, and noninfringement.

The maintainer does not guarantee that:
- File conversion will be error-free or uninterrupted across every file variant.
- Hardware-accelerated codecs will function identically across all GPU configurations.
- Compatibility with third-party software will remain unaffected by operating system updates.

Nothing in this section excludes or limits warranties or statutory rights that cannot legally be excluded under applicable consumer protection law.

## 7. Limitation of liability

To the maximum extent permitted by applicable law, neither the maintainer nor contributors will be liable for any direct, indirect, incidental, special, exemplary, or consequential damages (including loss of data, file corruption, device downtime, or lost profits) arising from the use of or inability to use Vertex.

This limitation does not apply to liabilities that cannot be disclaimed under applicable law, including intentional misconduct or gross negligence where prohibited.

## 8. Modifications to terms

The maintainer may update these terms to reflect new application features, legal standards, or operational practices. Updates will be documented in the repository commit history with an updated effective date.

## 9. Contact and notices

For security reports, questions, or legal notices, submit an issue at:  
https://github.com/rainaku/Vertex/issues
