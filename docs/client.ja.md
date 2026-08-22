[English](client.md) | **日本語**

# クライアントの設定

すべてのクエリは `Client` を経由します．自分で構築しなければ共有のデフォルト
クライアントが使われ，それだけで適切に振る舞います — 3 秒に 1 リクエスト，
説明的な `User-Agent`，上限つきのリトライ，上限つきのメモリ使用量．

これらを変更したいときは自分で構築してください．特に，**あなたのアプリケー
ションを名乗る `User-Agent` を設定することを推奨します**．

- [レート制限](#レート制限)
- [クライアントを構築する](#クライアントを構築する)
- [タイムアウトとリトライ](#タイムアウトとリトライ)
- [レスポンスサイズ](#レスポンスサイズ)
- [クライアントを共有する](#クライアントを共有する)

## レート制限

arXiv API の利用規約は，1 つの送信元から 3 秒に 1 リクエストを超えないよう
求めています．`Client` はこれを強制し，スケジュールはクローン間で共有される
ため，並行クエリはバーストせずキューイングされます．

待機は**到着順**に処理されます．待機中にキャンセルされた呼び出し
（`tokio::select!` で負けた分岐や abort されたタスク）がスケジュールを消費する
ことはありません．

arXiv は間隔とは別に**流量制限**も設けています．超過すると API が `429` を
返し，次の形で届きます．

```text
Error::Status { status: 429, retry_after: .., body: Some("Rate exceeded.") }
```

すぐに再試行せず，時間を置いてください．

## クライアントを構築する

```rust
use std::time::Duration;
use arxiv_tools::{ArXiv, Client, QueryParams};

#[tokio::main]
async fn main() -> Result<(), arxiv_tools::Error> {
    let client = Client::builder()
        .user_agent("my-app/1.0 (mailto:me@example.com)")
        .timeout(Duration::from_secs(60))
        .min_interval(Duration::from_secs(3))
        .max_retries(3)
        .build()?;

    let papers = client
        .fetch(&ArXiv::from_args(QueryParams::title("transformer")))
        .await?;

    println!("{} papers", papers.len());
    Ok(())
}
```

`Client` は `ArXiv` のメソッドと対応しています．

| `ArXiv` 側（共有クライアント） | `Client` 側 |
| --- | --- |
| `query()` | `fetch(&query)` |
| `query_page()` | `fetch_page(&query)` |
| `query_all(limit)` | `fetch_all(&query, limit)` |
| `Paper::download_pdf()` | `download_pdf(&paper)` |
| `Paper::download_pdf_to(path)` | `download_pdf_to(&paper, path)` |

## タイムアウトとリトライ

`5xx` と `429` のレスポンス，接続失敗，タイムアウトは `max_retries` 回まで
再試行されます（上限は `MAX_RETRIES_LIMIT`）．**レスポンス本文の読み取りを
含めた試行全体**が再試行対象です — 遅いフィードや大きな PDF では，接続拒否
よりも転送中の切断のほうが現実的だからです．

`Retry-After` は秒形式・HTTP-date 形式のどちらも，`MAX_RETRY_AFTER_WAIT` まで
**指定どおりに**尊重します．それを超える指定の場合は再試行せず，待ち時間を
`Error::Status` に載せて呼び出し元に返します — そこまで待つかどうかは呼び出し
元の判断だからです．`Retry-After` がない場合は `min_interval` から指数的に
バックオフします．

リトライは時間的にはただではありません．タイムアウトが続くリクエストは
`(max_retries + 1) × timeout` に試行間のバックオフを加えた時間 — 既定値では
約 2 分半 — かかってからエラーになります．早く失敗させたい場合は `timeout` か
`max_retries` を下げてください．

## レスポンスサイズ

本文は上限つきのチャンク単位で読み込むため，暴走した，あるいは悪意のある
レスポンスでメモリを使い切ることはありません．既定の上限は 64 MB で，2000 件の
フルページが占める約 30 MB に対して十分な余裕があります．これを超える本文は
`Error::ResponseTooLarge` になります．

```rust
use arxiv_tools::Client;

fn main() -> Result<(), arxiv_tools::Error> {
    let client = Client::builder()
        .max_response_size(256 * 1024 * 1024)
        .build()?;
    println!("{} bytes", client.max_response_size());
    Ok(())
}
```

`download_pdf_to` はディスクへストリーミングしますが，同じ上限に従います．

## クライアントを共有する

`Client` はコネクションプールとレートリミッタを保持します．クローン（安価
です）するか `Arc` で共有してください．**同じクライアントのクローンは 1 つの
リクエストスケジュールを共有します．** 別の `Client` を新たに構築すると
スケジュールも独立するため，2 つ合わせると arXiv が求める間隔を超え得ます．

## 既定値

| 定数 | 値 |
| --- | --- |
| `DEFAULT_MIN_INTERVAL` | 3 秒 |
| `DEFAULT_TIMEOUT` | 30 秒 |
| `DEFAULT_MAX_RETRIES` | 3 |
| `DEFAULT_MAX_RESPONSE_SIZE` | 64 MB |
| `MAX_RETRIES_LIMIT` | 10 |
| `MAX_RETRY_AFTER_WAIT` | 120 秒 |
| `MAX_MIN_INTERVAL` | 24 時間 |
| `MAX_RESULTS_PER_REQUEST` | 2000 |
