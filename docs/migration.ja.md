[English](migration.md) | **日本語**

# 1.x からの移行

2.0.0 では，旧シグネチャを保ったままでは直せなかった不具合をまとめて修正して
います．全一覧は [CHANGELOG.md](../CHANGELOG.md) にあります．このページでは
バージョンを上げたときに実際にぶつかる点を扱います．

## コンパイラが指摘してくれる変更

| 1.x | 2.0 |
| --- | --- |
| `arxiv.query().await` が `Vec<Paper>` を返した | `Result<Vec<Paper>, Error>` を返す |
| `let mut a = ArXiv::from_args(..); a.max_results(5);` | `ArXiv::from_args(..).max_results(5)` — ビルダーが `self` を受け取って返す |
| `arxiv.max_resutls` などの公開フィールド | フィールドは非公開．設定はビルダー，確認は `ArXiv::url()` |
| `paper.published2utc()` が `DateTime<Utc>` を返した | `Result<DateTime<Utc>, Error>` を返す |
| `paper.comment: Vec<String>` | `paper.comment: String` |
| `Category::Other(String)` | `Category::other("cs.NEW")?`（検証済みの `CategoryCode` を保持） |
| `Category::from(s)` | `s.parse::<Category>()?` または `Category::try_from(s)?` |
| `QueryParams::default()` | 廃止．`ArXiv::new()` か `ArXiv::from_id_list(..)` を使う |

1.x の呼び出し箇所：

```rust
// 1.x
// let mut arxiv = ArXiv::from_args(QueryParams::title("bert"));
// arxiv.max_results(5);
// let papers = arxiv.query().await;
// println!("{}", papers[0].title);
```

は次のようになります．

```rust
use arxiv_tools::{ArXiv, QueryParams};

#[tokio::main]
async fn main() -> Result<(), arxiv_tools::Error> {
    let papers = ArXiv::from_args(QueryParams::title("bert"))
        .max_results(5)
        .query()
        .await?;
    println!("{}", papers[0].title);
    Ok(())
}
```

## コンパイラが指摘しない変更

**拒否されたクエリがエラーになりました．** 1.x では arXiv が拒否したクエリが
`"Error"` というタイトルの `Paper` として返っていました．現在は arXiv 自身の
メッセージを載せた `Err(Error::Api { .. })` です．件数を数えていたコードは，
偽の論文 1 件ではなくエラーを受け取るようになります．

**`"` や `\` を含むフレーズは拒否されます．** 1.x はこれらを空白に置換し，
別の検索語で検索していました．引用符つきのフレーズをそのまま渡していた場合，
`ArXiv::url` から `Error::InvalidParam` が返ります — 引用符を外して単語で
検索してください．

**`journal_ref` は 1.x では全論文で空でした．** 回避策を入れていた場合は
外して構いません．

**`doi` の意味が変わりました．** 素の DOI（例：`10.1103/PhysRevD.76.013009`）
を保持します．`https://doi.org/...` のリンクは `doi_url` に移りました．

**入れ子の論理クエリの形が変わりました．** `and(vec![or(vec![a, b]), c])` は
1.x では `a OR b AND c` になっていましたが，現在は `(a OR b) AND c` です．
回避のために `group(..)` を挟んでいた場合，もう不要です（付けたままでも
動作します）．

**`Category` の直列化形式が変わりました．** Rust の enum 表現ではなく arXiv の
コード（`"cs.LG"`）になります．1.x の形式で永続化したデータは読み込めません．

**リクエストが 3 秒間隔になりました．** 連続してクエリを投げていたループは，
1 クエリあたり 3 秒かかるようになります．これは arXiv API の利用規約に沿った
挙動です．調整が必要な場合は[クライアントの設定](client.ja.md)を参照して
ください．

## 対応する最小の Rust バージョン

Rust 1.85（1.x では未指定）．
