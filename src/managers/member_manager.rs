//! Member registration and points.
//!
//! Ported from `App/managers/member_manager.py`. The Python manager opens a
//! registration window; here the logic is separated from the dialogs.

use std::sync::Arc;

use crate::db::{Member, SharedBackend};

use super::messages::{
    Message, MEMBER_ALREADY_EXISTS, MEMBER_REGISTER_FAILED, MEMBER_REGISTRATION, NOT_ENOUGH_POINTS,
    POINTS_REDEMPTION,
};
use super::rfid_cache::RfidCache;

/// Manages member registration and points.
pub struct MemberManager {
    db: SharedBackend,
    rfid_cache: Arc<RfidCache>,
}

impl MemberManager {
    /// A manager on top of a backend.
    pub fn new(db: SharedBackend) -> Self {
        MemberManager {
            db,
            rfid_cache: Arc::new(RfidCache::new()),
        }
    }

    /// A manager sharing an existing de-duplication cache with the table
    /// manager, so a scan made in one view is de-duplicated in the other.
    pub fn with_cache(db: SharedBackend, rfid_cache: Arc<RfidCache>) -> Self {
        MemberManager { db, rfid_cache }
    }

    /// The de-duplication cache.
    pub fn rfid_cache(&self) -> &Arc<RfidCache> {
        &self.rfid_cache
    }

    /// Register a new member.
    ///
    /// Mirrors the Python flow: the tag is checked first and the user is told
    /// that the member already exists.
    pub fn register_member(
        &self,
        rfid: &str,
        memberid: &str,
        name: &str,
        student_num: &str,
        program: &str,
        year: &str,
    ) -> Result<(), Message> {
        match self.db.member_exists(rfid) {
            Ok(Some(_)) => {
                return Err(Message::new(MEMBER_REGISTRATION, MEMBER_ALREADY_EXISTS));
            }
            Ok(None) => {}
            Err(e) => {
                tracing::error!("member lookup failed for {rfid}: {e}");
                return Err(Message::new(MEMBER_REGISTRATION, MEMBER_REGISTER_FAILED));
            }
        }

        match self
            .db
            .member_register(rfid, memberid, name, student_num, program, year)
        {
            Ok(()) => Ok(()),
            Err(e) => {
                tracing::error!("member registration failed for {rfid}: {e}");
                Err(Message::new(MEMBER_REGISTRATION, MEMBER_REGISTER_FAILED))
            }
        }
    }

    /// Look up a member by tag.
    pub fn member_exists(&self, rfid: &str) -> Result<Option<Member>, Message> {
        self.db.member_exists(rfid).map_err(|e| {
            tracing::error!("member lookup failed for {rfid}: {e}");
            Message::new(MEMBER_REGISTRATION, MEMBER_REGISTER_FAILED)
        })
    }

    /// Award points to a member.
    pub fn add_points(&self, rfid: &str, points: f64) -> Result<(), Message> {
        self.db.add_points(rfid, points).map_err(|e| {
            tracing::error!("adding points failed for {rfid}: {e}");
            Message::new(POINTS_REDEMPTION, format!("Could not add points: {e}"))
        })
    }

    /// The point balance of a member.
    pub fn points(&self, rfid: &str) -> Result<f64, Message> {
        self.db.get_member_points(rfid).map_err(|e| {
            tracing::error!("reading points failed for {rfid}: {e}");
            Message::new(POINTS_REDEMPTION, format!("Could not read points: {e}"))
        })
    }

    /// Spend points.
    ///
    /// The balance is checked first so the user gets a clear message instead
    /// of a silent no-op — the SQL refuses to go below zero as well.
    pub fn redeem_points(&self, rfid: &str, points: f64) -> Result<(), Message> {
        let balance = self.points(rfid)?;
        if balance < points {
            return Err(Message::new(POINTS_REDEMPTION, NOT_ENOUGH_POINTS));
        }
        self.db.redeem_points(rfid, points).map_err(|e| {
            tracing::error!("redeeming points failed for {rfid}: {e}");
            Message::new(POINTS_REDEMPTION, format!("Could not redeem points: {e}"))
        })
    }
}
