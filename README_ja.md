# OxLFS

[![Build](https://github.com/hayatek-jp/oxlfs/actions/workflows/build.yaml/badge.svg)](https://github.com/hayatek-jp/oxlfs/actions/workflows/build.yaml)
[![Test](https://github.com/hayatek-jp/oxlfs/actions/workflows/test.yaml/badge.svg)](https://github.com/hayatek-jp/oxlfs/actions/workflows/test.yaml)
[![oxlfs at crates.io](https://img.shields.io/crates/v/oxlfs.svg)](https://crates.io/crates/oxlfs)
[![oxlfs license](https://img.shields.io/crates/l/oxlfs.svg)](https://github.com/hayatek-jp/oxlfs/blob/main/LICENSE)

Rustで実装されたGit LFSサーバーです。OxLFSはGit LFS Batch APIとBasic転送アダプターに対応しており、オブジェクトをローカルファイルシステムに保存します。

> [!WARNING]
> OxLFSはベータ版のソフトウェアです。リリースによって設定や動作が変更される可能性があります。

> [!WARNING]
> 信頼できないクライアントから利用する場合は、必ずHTTPSを使用してください。

## 必要条件

- Rust 1.88以降
- GitおよびGit LFS
- OxLFSプロセスから書き込み可能なファイルシステム上のディレクトリ

## ビルド

```sh
cargo build --release
```

サーバーのバイナリは`target/release/oxlfs`に生成されます。`oxlfs_pw`ユーティリティも同時にビルドされ、Argon2形式のパスワードハッシュを生成できます。

## 設定

OxLFSを使用するには、TOML形式の設定ファイルが必要です。`--config`オプションで設定ファイルのパスを指定してください。

```sh
oxlfs --config /etc/oxlfs/config.toml
```

`--config`を指定しない場合は、ビルドの種類に応じて以下のパスが使用されます。

- デバッグビルド：`./sysroot/etc/oxlfs/config.toml`
- Windowsのリリースビルド：`C:\ProgramData\oxlfs\config.toml`
- その他のリリースビルド：`/etc/oxlfs/config.toml`

設定ファイルの例：

```toml
listen = "0.0.0.0:4443"
tls = true
tls_cert = "/etc/oxlfs/server.crt"
tls_key = "/etc/oxlfs/server.key"
# git_root = "repos"
storage_dir = "/var/lib/oxlfs/repos"
log_dir = "/var/log/oxlfs"
log_level = "info"
healthcheck_endpoint = true
jwt_secret = "replace-with-a-long-random-secret"
config_dir = "/etc/oxlfs"
```

`listen`、`tls`、`storage_dir`、`log_dir`、`jwt_secret`は必須項目です。TLSを有効にする場合は、`tls_cert`と`tls_key`も必須です。

`log_level`には`trace`、`debug`、`info`、`warn`、`error`、`off`のいずれかを指定できます。リリースビルドではデフォルト値として`info`が使用されます。

`healthcheck_endpoint`はデフォルトで`true`です。有効にすると、`GET /`でヘルスチェック用のエンドポイントが公開されます。

シークレットは、例えば次のコマンドで生成できます。

```sh
openssl rand -base64 32
```

`git_root`は、GitリポジトリのURLに使用するパスプレフィックスを指定します。例えば、リポジトリURLを`https://example.com/repos/<user>/<repo>.git`とする場合は、`git_root = "repos"`を設定します。

## ユーザーデータベース

サーバーは`config_dir`にある`users.toml`からユーザー情報を読み込みます。

設定例：

```toml
[public]
repos = ["alice/example"]

[[users]]
name = "alice"
password_hash = "$argon2id$..."
permissions = [
	{ repo = "alice/example", read = true, write = true },
	{ repo = "alice/*", read = true, write = false },
]
```

`public.repos`に登録されたリポジトリは、認証なしで読み取り可能になります。ただし、匿名での書き込みは常に禁止されます。

権限の対象には、特定のリポジトリ（`owner/repository`）または、特定の所有者が所有するすべてのリポジトリ（`owner/*`）を指定できます。

ユーザー名`anonymous`は予約語として使用されているため、ユーザー名として使用できません。

以下のコマンドでArgon2のパスワードハッシュを生成できます。

```sh
cargo run --bin oxlfs_pw -- 'correct horse battery staple'
```

出力されたハッシュを`password_hash`に設定してください。

Batch APIではHTTP Basic認証を使用します。一方、Batch APIから返されるアップロードURLおよびダウンロードURLでは、有効期間の短いBearer JWTが使用されます。

## Git LFSでの使用

```sh
git lfs install
git lfs track "*.bin"
git add .gitattributes
git commit -m "Track binary files with Git LFS"
git config lfs.url https://lfs.example.com/alice/example/info/lfs
git add model.bin
git commit -m "Add model"
git push origin main
```

`git_root`を設定しない場合、エンドポイントは次の形式になります。

```text
https://HOST/<user>/<repo>/info/lfs
```

`git_root = "repos"`を設定した場合：

```text
https://HOST/repos/<user>/<repo>/info/lfs
```

認証情報は、Gitの通常のcredential helperを使用して設定してください。

OxLFSはSHA-256オブジェクトIDと基本転送アダプターに対応しています。現在、以下の機能には対応していません。

- SSH認証
- その他の転送アダプター
- Batch APIの`verify`アクション

## ストレージとログ

オブジェクトは`storage_dir`以下に保存されます。オブジェクトはリポジトリごとに分けられ、さらにSHA-256オブジェクトIDの先頭2文字を使用してディレクトリ内で分類されます。

日次の`oxlfs.log`ファイルは`log_dir`以下に作成されます。また、ログは標準出力にも出力されます。

OxLFSを実行するサービスアカウントが、以下の操作を行えることを確認してください。

- `config_dir/users.toml`を読み取れること
- `storage_dir`に書き込めること
- `log_dir`に書き込めること

`jwt_secret`およびパスワードハッシュは、ソースコード管理システムに登録しないでください。

## ライセンス

OxLFSは[GNU Affero General Public License v3.0 only](LICENSE)の下でライセンスされています。
