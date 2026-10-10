# License and attribution notes

The `.agt` package embeds the Linux filesystem exported from the official `onlyoffice/documentserver` Community Docker image. The package includes ONLYOFFICE Docs and third-party components in `deps/`; their upstream binaries, web assets, license files, and notices are preserved from the image. The AgentOS launcher and workspace code are separate from those upstream components.

ONLYOFFICE Docs Community is distributed under AGPL-3.0 with additional attribution and branding terms. The packaged upstream notices are under `deps/var/www/onlyoffice/documentserver/LICENSE.txt` and `deps/var/www/onlyoffice/documentserver/3rd-Party.txt`; other bundled software may carry separate licenses. Anyone redistributing this `.agt` must review and satisfy the applicable source-offer, notice, attribution, and branding requirements for every included component.

Upstream references:

- [ONLYOFFICE Docs Community downloads and license](https://www.onlyoffice.com/en/download-community)
- [ONLYOFFICE Document Server source](https://github.com/ONLYOFFICE/DocumentServer)
- [Official Docker image source](https://github.com/ONLYOFFICE/Docker-DocumentServer)
- [ONLYOFFICE license FAQ](https://www.onlyoffice.com/license-faq)
