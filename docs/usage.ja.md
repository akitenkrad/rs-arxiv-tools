[English](usage.md) | **日本語**

# 使い方

以下はすべて非同期ランタイムを前提とします．例では `tokio` の `macros` と
`rt-multi-thread` フィーチャを有効にしています．

- [検索する](#検索する)
- [クエリを組み立てる](#クエリを組み立てる)
- [カテゴリ](#カテゴリ)
- [日付](#日付)
- [並び替え](#並び替え)
- [ID で取得する](#id-で取得する)
- [ページング](#ページング)
- [論文を読む](#論文を読む)
- [PDF をダウンロードする](#pdf-をダウンロードする)
- [エラー](#エラー)
- [拒否される入力](#拒否される入力)

## 検索する

arXiv の各検索フィールドに対応するコンストラクタが `QueryParams` にあります．

| コンストラクタ | arXiv のフィールド |
| --- | --- |
| `QueryParams::title` | `ti:` |
| `QueryParams::author` | `au:` |
| `QueryParams::abstract_text` | `abs:` |
| `QueryParams::comment` | `co:` |
| `QueryParams::journal_ref` | `jr:` |
| `QueryParams::report_number` | `rn:` |
| `QueryParams::subject_category` | `cat:` |
| `QueryParams::id` | `id:` |
| `QueryParams::all` | `all:` |

```rust
use arxiv_tools::{ArXiv, QueryParams};

#[tokio::main]
async fn main() -> Result<(), arxiv_tools::Error> {
    let papers = ArXiv::from_args(QueryParams::author("Yoshua Bengio"))
        .max_results(10)
        .query()
        .await?;

    for paper in &papers {
        println!("{} ({})", paper.title, paper.arxiv_id());
    }
    Ok(())
}
```

## クエリを組み立てる

`QueryParams` は式ツリーです．`and`，`or`，`and_not` がオペランドを結合し，
結合子が入れ子になった場合は**自動的に括弧が付きます**．そのためレンダリング
結果は arXiv 側の演算子優先順位に依存せず，組み立てたツリーどおりの意味に
なります．

```rust
use arxiv_tools::{ArXiv, Category, QueryParams};

#[tokio::main]
async fn main() -> Result<(), arxiv_tools::Error> {
    let args = QueryParams::and(vec![
        QueryParams::or(vec![
            QueryParams::title("ai"),
            QueryParams::title("llm"),
        ]),
        QueryParams::subject_category(Category::CsLg),
    ]);

    // -> (ti:"ai" OR ti:"llm") AND cat:"cs.LG"
    println!("{args}");

    let papers = ArXiv::from_args(args).max_results(50).query().await?;
    println!("{} papers", papers.len());
    Ok(())
}
```

明示的に括弧を付けたい場合は `QueryParams::group` を使います．1 つのグループに
複数のオペランドを入れた場合は `AND` で結合されます．

パーセントエンコーディングはリクエスト URL を組み立てる 1 回だけ行われます．
渡した文字列はそのまま arXiv に届きます．

`ArXiv::url` は送信せずにリクエスト全体を組み立てて返すので，クエリの確認に
便利です．

```rust
use arxiv_tools::{ArXiv, QueryParams};

fn main() -> Result<(), arxiv_tools::Error> {
    let url = ArXiv::from_args(QueryParams::title("attention is all you need"))
        .max_results(5)
        .url()?;
    println!("{url}");
    Ok(())
}
```

## カテゴリ

`Category` は arXiv の全分類を網羅しています — `cs.AI` から `math-ph`，
`q-bio.NC`，`econ.EM` まで 155 コードすべてです．

```rust
use arxiv_tools::Category;

fn main() -> Result<(), arxiv_tools::Error> {
    assert_eq!(Category::CsLg.as_str(), "cs.LG");
    assert_eq!("stat.ML".parse::<Category>()?, Category::StatMl);

    // 今後 arXiv が追加するコードも，カテゴリコードの形式であれば使えます．
    let future = Category::other("cs.FUTURE")?;
    assert_eq!(future.as_str(), "cs.FUTURE");

    // タイプミスは「何にもマッチしないカテゴリ」ではなくエラーになります．
    assert!("not a category".parse::<Category>().is_err());

    println!("{} known categories", Category::all().len());
    Ok(())
}
```

## 日付

`submittedDate` の範囲は arXiv 形式の `YYYYMMDDHHMM` 文字列か，任意の
タイムゾーンの `DateTime` で指定します．開いた範囲には `*` が使えます．
arXiv は分単位でインデックスするため，秒は切り捨てられます．

```rust
use arxiv_tools::QueryParams;
use chrono::{TimeZone, Utc};

fn main() {
    let from = Utc.with_ymd_and_hms(2024, 12, 1, 0, 0, 0).unwrap();
    let to = Utc.with_ymd_and_hms(2024, 12, 31, 23, 59, 0).unwrap();
    let range = QueryParams::submitted_between(from, to);

    assert_eq!(
        range.to_string(),
        "submittedDate:[202412010000 TO 202412312359]"
    );

    // arXiv 形式の文字列を直接渡すこともできます．
    let open_ended = QueryParams::submitted_date("202412010000", "*");
    assert_eq!(open_ended.to_string(), "submittedDate:[202412010000 TO *]");
}
```

## 並び替え

```rust
use arxiv_tools::{ArXiv, Category, QueryParams, SortBy, SortOrder};

#[tokio::main]
async fn main() -> Result<(), arxiv_tools::Error> {
    let papers = ArXiv::from_args(QueryParams::subject_category(Category::CsCl))
        .max_results(20)
        .sort_by(SortBy::SubmittedDate)
        .sort_order(SortOrder::Descending)
        .query()
        .await?;

    println!("newest: {}", papers[0].title);
    Ok(())
}
```

## ID で取得する

**1 要素につき 1 つの ID** を渡してください．新形式（`1706.03762v7`）と
旧形式（`hep-th/9901001`）の両方に対応しています．

```rust
use arxiv_tools::ArXiv;

#[tokio::main]
async fn main() -> Result<(), arxiv_tools::Error> {
    let papers = ArXiv::from_id_list(["1706.03762", "1810.04805"]).query().await?;
    assert_eq!(papers.len(), 2);
    Ok(())
}
```

`id_list` と `search_query` を併用すると，列挙した論文をそのクエリで絞り込み
ます．これは arXiv API の仕様どおりの挙動です．

## ページング

1 リクエストで返るのは最大 2000 件です．`query_page` はフィードが報告する
件数情報を返し，`query_all` は自動でページを辿ります．

```rust
use arxiv_tools::{ArXiv, Category, QueryParams};

#[tokio::main]
async fn main() -> Result<(), arxiv_tools::Error> {
    let query = ArXiv::from_args(QueryParams::subject_category(Category::CsLg));

    // 取得せずに件数だけ確認する．
    let page = query.clone().max_results(1).query_page().await?;
    println!("{} papers match", page.total_results);

    // 100 件ずつ，合計 500 件取得する．
    let papers = query.max_results(100).query_all(Some(500)).await?;
    println!("fetched {}", papers.len());
    Ok(())
}
```

`query_all` では `max_results` が **1 ページあたりの件数**，`limit` 引数が
**合計件数**です．`None` を渡すと全件取得になりますが，返るまで全件をメモリ上に
保持するため，先に `total_results` を確認してください．

リクエストは 3 秒間隔で送られるため，大きな `query_all` は設計上時間がかかり
ます．[クライアントの設定](client.ja.md)を参照してください．

## 論文を読む

```rust
use arxiv_tools::{ArXiv, Category};

#[tokio::main]
async fn main() -> Result<(), arxiv_tools::Error> {
    let papers = ArXiv::from_id_list(["0704.0001"]).query().await?;
    let paper = &papers[0];

    println!("{}", paper.title);
    println!("{}", paper.authors.join(", "));
    println!("{}", paper.abstract_text);

    println!("{}", paper.arxiv_id());               // 0704.0001v2
    println!("{:?}", paper.version());              // Some(2)
    println!("{}", paper.published2utc()?);         // パース済みのタイムスタンプ
    println!("{}", paper.doi);                      // 10.1103/PhysRevD.76.013009
    println!("{}", paper.journal_ref);              // Phys.Rev.D76:013009,2007
    println!("{}", paper.pdf_url);

    println!("{}", paper.has_category(&Category::CsLg));
    Ok(())
}
```

カテゴリとタイムスタンプは arXiv が返した文字列のまま保持します．見慣れない
カテゴリコードや不正な日付があっても，論文全体のパースが失敗しないようにする
ためです．型付きの `Category` との比較には `Paper::has_category` を，
タイムスタンプのパースには `published2utc` / `updated2utc` を使ってください．

## PDF をダウンロードする

```rust
use arxiv_tools::ArXiv;

#[tokio::main]
async fn main() -> Result<(), arxiv_tools::Error> {
    let papers = ArXiv::from_id_list(["1706.03762"]).query().await?;

    // ストリーミングでディスクへ直接保存する．
    let written = papers[0].download_pdf_to("attention.pdf").await?;
    println!("wrote {written} bytes");

    // メモリ上に取得することもできる．
    let bytes = papers[0].download_pdf().await?;
    println!("{} bytes", bytes.len());
    Ok(())
}
```

ダウンロードはクエリとレートリミッタを共有するため，まとめて取得しても arXiv の
利用規約の範囲に収まります．`download_pdf_to` は保存先の隣に一意な名前の一時
ファイルを作ってストリーミングし，本文が届いてから rename します．そのため失敗
やキャンセルで切り詰められたファイルが残ることはなく，同じパスへの並行
ダウンロードが混ざることもありません．

arXiv は PDF の生成中に HTML の通知ページを返します．`Content-Type` と
`%PDF-` シグネチャの両方を検証しているため，これは `.pdf` という名前の HTML
ファイルとして保存されるのではなく `Error::UnexpectedContentType` になります．

## エラー

| バリアント | 意味 |
| --- | --- |
| `Error::Api` | arXiv がクエリを拒否し，理由を返した |
| `Error::Status` | 非成功の HTTP ステータス（`Retry-After` と本文抜粋つき） |
| `Error::Http` | リクエストが完了しなかった（DNS，TLS，タイムアウト） |
| `Error::Parse` | レスポンスが整形式かつ完全な Atom フィードではなかった |
| `Error::ResponseTooLarge` | レスポンスがクライアントのバッファ上限を超えた |
| `Error::InvalidTimestamp` | 日付フィールドが RFC 3339 ではなかった |
| `Error::InvalidParam` | 送信不可能なクエリ（リクエスト前に検出） |
| `Error::UnexpectedContentType` | ダウンロード結果が PDF ではなかった |
| `Error::Io` | ダウンロードしたファイルを書き込めなかった |
| `Error::ClientInit` | HTTP クライアントを構築できなかった |

検索結果 0 件はエラーではなく `Ok(vec![])` です．一方，切り詰められた
レスポンス，Atom フィードでないレスポンス，ページング件数を欠くレスポンスは
いずれも `Error::Parse` になります — 黙って短い結果を返すことはありません．

## 拒否される入力

`QueryParams::validate` は `ArXiv::url` から呼ばれるため，以下はリクエストを
送る前に検出されます．

- 空のフレーズ
- `"` または `\` を含むフレーズ．arXiv は検索フィールドを二重引用符で囲む
  仕様で，エスケープ手段が定義されていないため表現できません．**黙って別の
  検索語に書き換えるのではなくエラーにします**
- `YYYYMMDDHHMM` として実在しない `submittedDate` の境界値，`*` 以外の不正な値，
  および開始が終了より後の範囲
- オペランドが 0 個の `AND` / `OR`，オペランドが 2 個未満の `ANDNOT`
- `max_results` が 0，または 2000 超
- arXiv 識別子の形をしていない `id_list` の要素．特にカンマを含む要素は，
  そのままでは複数の識別子として API に届いてしまうため拒否します

検証を通ったクエリはツリー全体がそのままレンダリングされます．オペランドが
黙って除去されたり結合子が消えたりしないため，組み立てた式がそのまま arXiv に
渡ります．
