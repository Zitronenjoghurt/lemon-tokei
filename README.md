# Badge Service

A badge service for displaying repository statistics. Forked from [tokei.rs](https://github.com/XAMPPRocky/tokei_rs) by
XAMPPRocky.

## Deployment

```yaml
services:
  lemon-tokei:
    container_name: lemon-tokei
    image: ghcr.io/zitronenjoghurt/lemon-tokei:latest
    env_file:
      - .env
    restart: unless-stopped
```

```dotenv
# The port to run the server on, will default to 8000 if not set.
TOKEI_PORT=8000
# Comma-separated or empty, if empty or not set, all users are allowed.
ALLOWED_USERS=Zitronenjoghurt,XAMPPRocky
```

## Scheme

The URL scheme is as follows:

```
{BASE_URL}/b1/<domain>[<.com>]?/<namespace>/<repository>
```

- `domain`: The domain name of the git host. If no TLD is provided, `.com` is added.
  e.g. `{BASE_URL}/b1/github` == `{BASE_URL}/b1/github.com`.
- `namespace`: The namespace of the repo, e.g. `rust-lang` or `XAMPPRocky`.
- `repository`: The name of the repo, e.g. `rust` or `tokei`.

## Usage

By default the badge shows the repo's total lines. The sections below cover the available query parameters for
customisation.

### Category

Specify a different category with the `?category=` query string. Supported values: `code`, `blanks`, `files`, `lines`,
`comments`.

```
{BASE_URL}/b1/github/XAMPPRocky/tokei?category=code
```

### Type

Count lines only for specific language type(s) with `?type=`. Separate multiple languages with a comma.

```
{BASE_URL}/b1/github/XAMPPRocky/tokei?type=JSON,Rust,Markdown
```

### Branch

Count lines from a specific branch with `?branch=`. If omitted, the default `HEAD` branch is used.

```
{BASE_URL}/b1/github/rust-lang/rust?branch=beta
```

### Label

Customise the badge label with `?label=`.

```
{BASE_URL}/b1/github/XAMPPRocky/tokei?category=code&label=custom%20label
```

### Style

Customise the badge style with `?style=`. Supported styles: `flat` (default), `flat-square`, `plastic`, `for-the-badge`,
`social`.

```
{BASE_URL}/b1/github/XAMPPRocky/tokei?category=code&style=for-the-badge
```

### Color

Customise the badge color with `?color=`. Supports named colors and RGB hexadecimal. The default is blue (`#007ec6`). A
full list of supported formats can be found [here](https://crates.io/crates/csscolorparser).

```
{BASE_URL}/b1/github/XAMPPRocky/tokei?category=code&color=ff0000
```

### Logo

Add a custom logo (SVG format) by passing its full URL to `?logo=`.

```
{BASE_URL}/b1/github/XAMPPRocky/tokei?category=code&logo=https://simpleicons.org/icons/rust.svg
```

### Most Used Language

Display the name of the n-th most used language by enabling `?showLanguage=true` and setting `?languageRank=` (e.g. `1`
for most used, `2` for second most, etc.).

```
{BASE_URL}/b1/github/XAMPPRocky/tokei?showLanguage=true&languageRank=1&label=Most%20Used%20Language
{BASE_URL}/b1/github/XAMPPRocky/tokei?showLanguage=true&languageRank=2&label=2nd%20Most%20Used%20Language
{BASE_URL}/b1/github/XAMPPRocky/tokei?showLanguage=true&languageRank=3&label=3rd%20Most%20Used%20Language
```

## License

Licensed under either of Apache License 2.0 or MIT license, at your option.