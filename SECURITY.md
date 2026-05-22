# Security Policy

## Supported Versions

SpriteAnimte is currently in early development. Security fixes target the latest commit on the main development branch unless a release branch is explicitly created.

## Reporting A Vulnerability

Please do not open public issues for vulnerabilities involving secrets, local file access, API credentials, generated media paths, or packaging behavior.

Report privately through GitHub's private vulnerability reporting feature if it is enabled for this repository. If it is not enabled, contact the repository owner directly and include:

- a clear description of the issue;
- steps to reproduce;
- affected platform and build type;
- whether local config, API keys, generated files, or external services are involved.

## Sensitive Data

Do not share real `SpriteAnimteData/config.json` files, API keys, proxy URLs, generated logs, or private media in bug reports. Use redacted examples and placeholders such as `https://your-api.example/v1`.
