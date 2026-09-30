//! Database row types and query filters.

/// A group ("dictionary").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Group {
    /// Primary key.
    pub id: i64,
    /// Unique, non-empty name.
    pub name: String,
    /// Creation time, unix seconds UTC.
    pub created_at: i64,
}

/// A flashcard. Every card belongs to exactly one group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Card {
    /// Primary key.
    pub id: i64,
    /// Front side (the source word).
    pub front: String,
    /// Back side (translation or definition); may be empty.
    pub back: String,
    /// Owning group; never null.
    pub group_id: i64,
    /// Creation time, unix seconds UTC.
    pub created_at: i64,
    /// Last modification time, unix seconds UTC.
    pub updated_at: i64,
}

/// Optional bounds for card queries. `None` means "no bound".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CardFilter {
    /// Restrict to a single group. [`crate::db::cards::list_by_group`] forces
    /// this to its explicit `group_id` argument.
    pub group_id: Option<i64>,
    /// Inclusive lower bound on `created_at` (unix seconds).
    pub from: Option<i64>,
    /// Inclusive upper bound on `created_at` (unix seconds).
    pub to: Option<i64>,
}

/// One side of a language pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// English.
    En,
    /// Russian.
    Ru,
}

impl Lang {
    /// Human-readable English name, for use in LLM system prompts.
    pub fn name(self) -> &'static str {
        match self {
            Lang::En => "English",
            Lang::Ru => "Russian",
        }
    }
}

/// A supported source/target language pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LangPair {
    /// English to English (definition).
    EnEn,
    /// English to Russian.
    EnRu,
    /// Russian to English.
    RuEn,
}

impl LangPair {
    /// The language the source word is written in.
    pub fn source(self) -> Lang {
        match self {
            LangPair::EnEn | LangPair::EnRu => Lang::En,
            LangPair::RuEn => Lang::Ru,
        }
    }

    /// The language of the translation/definition.
    pub fn target(self) -> Lang {
        match self {
            LangPair::EnEn => Lang::En,
            LangPair::EnRu => Lang::Ru,
            LangPair::RuEn => Lang::En,
        }
    }

    /// `(source, target)` human-readable names.
    pub fn lang_names(self) -> (&'static str, &'static str) {
        (self.source().name(), self.target().name())
    }

    /// Canonical code, matching the configuration spelling (`en-en`, ...).
    pub fn code(self) -> &'static str {
        match self {
            LangPair::EnEn => "en-en",
            LangPair::EnRu => "en-ru",
            LangPair::RuEn => "ru-en",
        }
    }
}

impl From<crate::config::LanguagePair> for LangPair {
    fn from(pair: crate::config::LanguagePair) -> Self {
        match pair {
            crate::config::LanguagePair::EnEn => LangPair::EnEn,
            crate::config::LanguagePair::EnRu => LangPair::EnRu,
            crate::config::LanguagePair::RuEn => LangPair::RuEn,
        }
    }
}
