//! Review session: a shuffled snapshot of a group's cards.
//!
//! The session is a plain in-memory list; it is never written back to the
//! database. [`build_session`] fetches the cards with the same filter/limit as
//! the group view and shuffles them with a seeded RNG so it can be tested.

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use rusqlite::Connection;

use crate::db::{self, cards, Card, CardFilter};

/// A shuffled, finite sequence of cards to go through.
#[derive(Debug, Clone)]
pub struct Session {
    cards: Vec<Card>,
    index: usize,
    revealed: bool,
}

impl Session {
    /// Build a session from an already-fetched list.
    pub fn from_cards(mut cards: Vec<Card>, seed: u64) -> Session {
        shuffle_with_seed(&mut cards, seed);
        Session {
            cards,
            index: 0,
            revealed: false,
        }
    }

    /// All cards in the session, in presentation order.
    pub fn cards(&self) -> &[Card] {
        &self.cards
    }

    /// Number of cards in the session.
    pub fn len(&self) -> usize {
        self.cards.len()
    }

    /// Whether the session has no cards.
    pub fn is_empty(&self) -> bool {
        self.cards.is_empty()
    }

    /// 1-based position of the current card.
    pub fn position(&self) -> usize {
        (self.index + 1).min(self.cards.len().max(1))
    }

    /// The card under the cursor, if any.
    pub fn current(&self) -> Option<&Card> {
        self.cards.get(self.index)
    }

    /// Whether the back side of the current card is visible.
    pub fn is_revealed(&self) -> bool {
        self.revealed
    }

    /// Reveal the back side of the current card.
    pub fn reveal(&mut self) {
        self.revealed = true;
    }

    /// Move to the next card (hiding its back).
    pub fn advance(&mut self) {
        self.index += 1;
        self.revealed = false;
    }

    /// Whether every card has been shown.
    pub fn is_finished(&self) -> bool {
        self.index >= self.cards.len()
    }
}

/// Fetch and shuffle the cards of `group_id`.
///
/// `limit` is applied by the query; the result therefore contains at most
/// `limit` cards, matching the group view's selection rules.
pub fn build_session(
    conn: &Connection,
    group_id: i64,
    filter: &CardFilter,
    limit: usize,
    seed: u64,
) -> db::Result<Session> {
    let cards = cards::review_query(conn, group_id, filter, limit)?;
    Ok(Session::from_cards(cards, seed))
}

/// Shuffle `cards` deterministically for the given `seed`.
pub fn shuffle_with_seed(cards: &mut [Card], seed: u64) {
    let mut rng = StdRng::seed_from_u64(seed);
    cards.shuffle(&mut rng);
}
