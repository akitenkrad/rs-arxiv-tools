use percent_encoding::utf8_percent_encode;
use percent_encoding::NON_ALPHANUMERIC as NON_ALNUM;
use quick_xml::events::Event;
use quick_xml::reader::Reader;
use reqwest as request;
use serde::{Deserialize, Serialize};

fn encode(s: &str) -> String {
    utf8_percent_encode(s, NON_ALNUM)
        .to_string()
        .replace("%20", "+")
}

pub enum PrimaryCategory {
    CsAi(String),
    CsCl(String),
    CsLg(String),
    CsGt(String),
    CsCv(String),
    CsCr(String),
    CsCc(String),
    CsCe(String),
    CsCy(String),
    CsDs(String),
    CsDm(String),
    CsDc(String),
    CsEt(String),
    CsFl(String),
    CsGl(String),
    CsGr(String),
    CsAr(String),
    CsHc(String),
    CsIr(String),
}

impl PrimaryCategory {
    pub fn csai() -> Self {
        PrimaryCategory::CsAi("cs.AI".to_string())
    }
    pub fn cscl() -> Self {
        PrimaryCategory::CsCl("cs.CL".to_string())
    }
    pub fn cslg() -> Self {
        PrimaryCategory::CsLg("cs.LG".to_string())
    }
    pub fn csgt() -> Self {
        PrimaryCategory::CsGt("cs.GT".to_string())
    }
    pub fn cscv() -> Self {
        PrimaryCategory::CsCv("cs.CV".to_string())
    }
    pub fn cscr() -> Self {
        PrimaryCategory::CsCr("cs.CR".to_string())
    }
    pub fn cscc() -> Self {
        PrimaryCategory::CsCc("cs.CC".to_string())
    }
    pub fn csce() -> Self {
        PrimaryCategory::CsCe("cs.CE".to_string())
    }
    pub fn cscy() -> Self {
        PrimaryCategory::CsCy("cs.CY".to_string())
    }
    pub fn csds() -> Self {
        PrimaryCategory::CsDs("cs.DS".to_string())
    }
    pub fn csdm() -> Self {
        PrimaryCategory::CsDm("cs.DM".to_string())
    }
    pub fn csdc() -> Self {
        PrimaryCategory::CsDc("cs.DC".to_string())
    }
    pub fn cset() -> Self {
        PrimaryCategory::CsEt("cs.ET".to_string())
    }
    pub fn csfl() -> Self {
        PrimaryCategory::CsFl("cs.FL".to_string())
    }
    pub fn csgr() -> Self {
        PrimaryCategory::CsGr("cs.GR".to_string())
    }
    pub fn csar() -> Self {
        PrimaryCategory::CsAr("cs.AR".to_string())
    }
    pub fn cshc() -> Self {
        PrimaryCategory::CsHc("cs.HC".to_string())
    }
    pub fn csir() -> Self {
        PrimaryCategory::CsIr("cs.IR".to_string())
    }
}

pub enum ArXivArgs {
    Title(String),
    Author(String),
    Abstract(String),
    Comment(String),
    JournalRef(String),
    SubjectCategory(String),
    ReportNumber(String),
    SubmittedDate(String),
    Id(String),
    All(String),
    And(String),
    Or(String),
    AndNot(String),
    GroupStart(String),
    GroupEnd(String),
}

impl ArXivArgs {
    pub fn title(arg: String) -> Self {
        return ArXivArgs::Title(encode(&format!("ti:\"{}\"", arg)));
    }
    pub fn author(arg: String) -> Self {
        return ArXivArgs::Author(encode(&format!("au:\"{}\"", arg)));
    }
    pub fn abstract_text(arg: String) -> Self {
        return ArXivArgs::Abstract(encode(&format!("abs:\"{}\"", arg)));
    }
    pub fn comment(arg: String) -> Self {
        return ArXivArgs::Comment(encode(&format!("co:\"{}\"", arg)));
    }
    pub fn journal_ref(arg: String) -> Self {
        return ArXivArgs::JournalRef(encode(&format!("jr:\"{}\"", arg)));
    }
    pub fn subject_category(arg: String) -> Self {
        return ArXivArgs::SubjectCategory(encode(&format!("cat:\"{}\"", arg)));
    }
    pub fn report_number(arg: String) -> Self {
        return ArXivArgs::ReportNumber(encode(&format!("rn:\"{}\"", arg)));
    }
    pub fn id(id: String) -> Self {
        return ArXivArgs::Id(encode(&format!("id:\"{}\"", id)));
    }
    pub fn all(arg: String) -> Self {
        return ArXivArgs::All(encode(&format!("all:\"{}\"", arg)));
    }
    pub fn submitted_date(arg_from: String, arg_to: String) -> Self {
        return ArXivArgs::SubmittedDate(format!("&submittedDate:[{}+TO+{}]", arg_from, arg_to));
    }
    pub fn and() -> Self {
        return ArXivArgs::And(encode(" AND "));
    }
    pub fn or() -> Self {
        return ArXivArgs::Or(encode(" OR "));
    }
    pub fn and_not() -> Self {
        return ArXivArgs::AndNot(encode(" ANDNOT "));
    }
    pub fn group_start() -> Self {
        return ArXivArgs::GroupStart(encode("("));
    }
    pub fn group_end() -> Self {
        return ArXivArgs::GroupEnd(encode(")"));
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ArXivResponse {
    pub id: String,
    pub title: String,
    pub authors: Vec<String>,
    #[serde(rename = "abstract")]
    pub abstract_text: String,
    pub published: String,
    pub updated: String,
    pub doi: String,
    pub comment: Vec<String>,
    pub journal_ref: String,
    pub pdf_url: String,
    pub primary_category: String,
    pub categories: Vec<String>,
}

impl ArXivResponse {
    pub fn default() -> Self {
        return ArXivResponse {
            id: "".to_string(),
            title: "".to_string(),
            authors: Vec::new(),
            abstract_text: "".to_string(),
            published: "".to_string(),
            updated: "".to_string(),
            doi: "".to_string(),
            comment: Vec::new(),
            journal_ref: "".to_string(),
            pdf_url: "".to_string(),
            primary_category: "".to_string(),
            categories: Vec::new(),
        };
    }
}

pub struct ArXiv {
    pub url: String,
    pub base_url: String,
    pub args: Vec<ArXivArgs>,
}

impl ArXiv {
    pub fn new() -> Self {
        return ArXiv {
            base_url: "http://export.arxiv.org/api/query?search_query=".to_string(),
            url: "".to_string(),
            args: Vec::new(),
        };
    }

    pub fn title(&mut self, title: &str) -> &mut Self {
        self.args.push(ArXivArgs::title(title.to_string()));
        return self;
    }

    pub fn author(&mut self, author: &str) -> &mut Self {
        self.args.push(ArXivArgs::author(author.to_string()));
        return self;
    }

    pub fn abstract_text(&mut self, abstract_text: &str) -> &mut Self {
        self.args
            .push(ArXivArgs::abstract_text(abstract_text.to_string()));
        return self;
    }

    pub fn comment(&mut self, comment: &str) -> &mut Self {
        self.args.push(ArXivArgs::comment(comment.to_string()));
        return self;
    }

    pub fn journal_ref(&mut self, journal_ref: &str) -> &mut Self {
        self.args
            .push(ArXivArgs::journal_ref(journal_ref.to_string()));
        return self;
    }

    pub fn subject_category(&mut self, category: &str) -> &mut Self {
        self.args
            .push(ArXivArgs::subject_category(category.to_string()));
        return self;
    }

    pub fn report_number(&mut self, report_number: &str) -> &mut Self {
        self.args
            .push(ArXivArgs::report_number(report_number.to_string()));
        return self;
    }

    pub fn id(&mut self, id: &str) -> &mut Self {
        self.args.push(ArXivArgs::id(id.to_string()));
        return self;
    }

    pub fn and(&mut self) -> &mut Self {
        self.args.push(ArXivArgs::and());
        return self;
    }

    pub fn or(&mut self) -> &mut Self {
        self.args.push(ArXivArgs::or());
        return self;
    }

    pub fn and_not(&mut self) -> &mut Self {
        self.args.push(ArXivArgs::and_not());
        return self;
    }

    pub fn group_start(&mut self) -> &mut Self {
        self.args.push(ArXivArgs::group_start());
        return self;
    }

    pub fn group_end(&mut self) -> &mut Self {
        self.args.push(ArXivArgs::group_end());
        return self;
    }

    pub fn submitted_date(&mut self, from: &str, to: &str) -> &mut Self {
        self.args
            .push(ArXivArgs::submitted_date(from.to_string(), to.to_string()));
        return self;
    }

    fn parse_xml(&self, xml: String) -> Vec<ArXivResponse> {
        let mut reader = Reader::from_str(&xml);
        let mut buf = Vec::new();
        let mut in_entry = false;
        let mut in_id = false;
        let mut in_title = false;
        let mut in_author = false;
        let mut in_name = false;
        let mut in_abstract = false;
        let mut in_published = false;
        let mut in_updated = false;
        let mut in_comment = false;
        let mut in_journal_ref = false;

        let mut responses: Vec<ArXivResponse> = Vec::new();
        let mut res = ArXivResponse::default();
        loop {
            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(ref e)) => {
                    if e.name().as_ref() == b"entry" {
                        in_entry = true;
                        res = ArXivResponse::default();
                    } else if e.name().as_ref() == b"id" {
                        in_id = true;
                    } else if e.name().as_ref() == b"title" {
                        in_title = true;
                    } else if e.name().as_ref() == b"author" {
                        in_author = true;
                    } else if e.name().as_ref() == b"name" {
                        if in_author {
                            in_name = true;
                        }
                    } else if e.name().as_ref() == b"summary" {
                        in_abstract = true;
                    } else if e.name().as_ref() == b"published" {
                        in_published = true;
                    } else if e.name().as_ref() == b"updated" {
                        in_updated = true;
                    } else if e.name().as_ref() == b"arxiv:comment" {
                        in_comment = true;
                    } else if e.name().as_ref() == b"arxiv:journal_ref" {
                        in_journal_ref = true;
                    } else if e.name().as_ref() == b"link" && in_entry {
                        let mut is_pdf = false;
                        let mut is_doi = false;
                        e.attributes().for_each(|attr| {
                            if let Ok(attr) = attr {
                                if attr.key.as_ref() == b"title" && attr.value.as_ref() == b"pdf" {
                                    is_pdf = true;
                                } else if attr.key.as_ref() == b"title"
                                    && attr.value.as_ref() == b"doi"
                                {
                                    is_doi = true;
                                }
                            }
                        });
                        e.attributes().for_each(|attr| {
                            if let Ok(attr) = attr {
                                if attr.key.as_ref() == b"href" {
                                    if is_pdf {
                                        res.pdf_url = String::from_utf8_lossy(attr.value.as_ref())
                                            .to_string();
                                    } else if is_doi {
                                        res.doi = String::from_utf8_lossy(attr.value.as_ref())
                                            .to_string();
                                    }
                                }
                            }
                        });
                    } else if e.name().as_ref() == b"arxiv:primary_category" {
                        e.attributes().for_each(|attr| {
                            if let Ok(attr) = attr {
                                if attr.key.as_ref() == b"term" {
                                    res.primary_category =
                                        String::from_utf8_lossy(attr.value.as_ref()).to_string();
                                }
                            }
                        });
                    } else if e.name().as_ref() == b"category" {
                        if let Some(attr) = e
                            .attributes()
                            .find(|attr| attr.as_ref().unwrap().key.as_ref() == b"term")
                        {
                            res.categories.push(
                                String::from_utf8_lossy(attr.unwrap().value.as_ref()).to_string(),
                            );
                        }
                    } else if e.name().as_ref() == b"category" {
                        if let Some(attr) = e
                            .attributes()
                            .find(|attr| attr.as_ref().unwrap().key.as_ref() == b"term")
                        {
                            res.categories.push(
                                String::from_utf8_lossy(attr.unwrap().value.as_ref()).to_string(),
                            );
                        }
                    }
                }
                Ok(Event::End(ref e)) => {
                    if e.name().as_ref() == b"entry" {
                        in_entry = false;
                        responses.push(res.clone());
                        res = ArXivResponse::default();
                    } else if e.name().as_ref() == b"id" {
                        in_id = false;
                    } else if e.name().as_ref() == b"title" {
                        in_title = false;
                    } else if e.name().as_ref() == b"author" {
                        in_author = false;
                    } else if e.name().as_ref() == b"name" {
                        if in_author {
                            in_name = false;
                        }
                    } else if e.name().as_ref() == b"summary" {
                        in_abstract = false;
                    } else if e.name().as_ref() == b"published" {
                        in_published = false;
                    } else if e.name().as_ref() == b"updated" {
                        in_updated = false;
                    } else if e.name().as_ref() == b"arxiv:comment" {
                        in_comment = false;
                    } else if e.name().as_ref() == b"arxiv:journal_ref" {
                        in_journal_ref = true;
                    }
                }
                Ok(Event::Text(e)) => {
                    if in_entry {
                        if in_id {
                            res.id = e.unescape().unwrap().to_string();
                        } else if in_title {
                            res.title = e.unescape().unwrap().to_string();
                        } else if in_author && in_name {
                            res.authors.push(e.unescape().unwrap().to_string());
                        } else if in_abstract {
                            res.abstract_text =
                                e.unescape().unwrap().to_string().trim().replace("\n", "");
                        } else if in_published {
                            res.published = e.unescape().unwrap().to_string();
                        } else if in_updated {
                            res.updated = e.unescape().unwrap().to_string();
                        } else if in_comment {
                            res.comment.push(e.unescape().unwrap().to_string());
                        } else if in_journal_ref {
                            res.journal_ref = e.unescape().unwrap().to_string();
                        }
                    }
                }
                Ok(Event::Empty(ref e)) => {
                    if e.name().as_ref() == b"link" && in_entry {
                        let mut is_pdf = false;
                        let mut is_doi = false;
                        e.attributes().for_each(|attr| {
                            if let Ok(attr) = attr {
                                if attr.key.as_ref() == b"title" && attr.value.as_ref() == b"pdf" {
                                    is_pdf = true;
                                } else if attr.key.as_ref() == b"title"
                                    && attr.value.as_ref() == b"doi"
                                {
                                    is_doi = true;
                                }
                            }
                        });
                        e.attributes().for_each(|attr| {
                            if let Ok(attr) = attr {
                                if attr.key.as_ref() == b"href" {
                                    if is_pdf {
                                        res.pdf_url = String::from_utf8_lossy(attr.value.as_ref())
                                            .to_string();
                                    } else if is_doi {
                                        res.doi = String::from_utf8_lossy(attr.value.as_ref())
                                            .to_string();
                                    }
                                }
                            }
                        });
                    } else if e.name().as_ref() == b"arxiv:primary_category" && in_entry {
                        e.attributes().for_each(|attr| {
                            if let Ok(attr) = attr {
                                if attr.key.as_ref() == b"term" {
                                    res.primary_category =
                                        String::from_utf8_lossy(attr.value.as_ref()).to_string();
                                }
                            }
                        });
                    } else if e.name().as_ref() == b"category" && in_entry {
                        if let Some(attr) = e
                            .attributes()
                            .find(|attr| attr.as_ref().unwrap().key.as_ref() == b"term")
                        {
                            res.categories.push(
                                String::from_utf8_lossy(attr.unwrap().value.as_ref()).to_string(),
                            );
                        }
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => panic!("Error at position {}: {:?}", reader.buffer_position(), e),
                _ => (),
            }
            buf.clear();
        }
        return responses;
    }

    pub async fn query(&mut self) -> Vec<ArXivResponse> {
        let mut query = "".to_string();
        for arg in &self.args {
            match arg {
                ArXivArgs::Title(arg) => query.push_str(&arg),
                ArXivArgs::Author(arg) => query.push_str(&arg),
                ArXivArgs::Abstract(arg) => query.push_str(&arg),
                ArXivArgs::Comment(arg) => query.push_str(&arg),
                ArXivArgs::JournalRef(arg) => query.push_str(&arg),
                ArXivArgs::SubjectCategory(arg) => query.push_str(&arg),
                ArXivArgs::ReportNumber(arg) => query.push_str(&arg),
                ArXivArgs::SubmittedDate(arg) => query.push_str(&arg),
                ArXivArgs::Id(arg) => query.push_str(&arg),
                ArXivArgs::All(arg) => query.push_str(&arg),
                ArXivArgs::And(arg) => query.push_str(&arg),
                ArXivArgs::Or(arg) => query.push_str(&arg),
                ArXivArgs::AndNot(arg) => query.push_str(&arg),
                ArXivArgs::GroupStart(arg) => query.push_str(&arg),
                ArXivArgs::GroupEnd(arg) => query.push_str(&arg),
            }
        }
        self.url = format!(
            "{}{}&sortBy=lastUpdatedDate&sortOrder=descending",
            self.base_url, query,
        );
        self.url = self.url.replace("%20", "+");

        let body = request::get(&self.url).await.unwrap().text().await.unwrap();
        let responses = self.parse_xml(body);
        return responses;
    }
}

#[cfg(test)]
mod tests;
