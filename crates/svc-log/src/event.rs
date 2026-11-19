//! The typed events read out of `Game.log`.
//!
//! Every variant is grounded in a line seen in a real 4.x log; the line is
//! quoted on the parser that recognises it ([`crate::parse`]) and used
//! verbatim as that parser's test fixture.
//!
//! # Locale: some events only fire with English game text
//!
//! The events read from **HUD notifications** match the notification's
//! English wording (`Contract Accepted: `, `Awarded … aUEC`, …). The game
//! writes the localized string into the log, so with a language pack that
//! translates those strings (Starlume's own Text Patching can install one)
//! they silently never fire. They are marked **locale-dependent** below:
//! [`LogEventKind::Contract`], [`LogEventKind::Objective`],
//! [`LogEventKind::Money`], [`LogEventKind::RefineryOrderCompleted`],
//! [`LogEventKind::BlueprintReceived`], [`LogEventKind::JurisdictionEntered`].
//! Everything else is read from engine lines, which are not localized.
//!
//! Not solved here. The proper fix: derive the prefixes from the active
//! `global.ini` instead of hard-coding English, i.e. resolve the
//! notification's localization keys against the install's current text
//! (after patching) and hand the parser that table; the engine-side lines
//! (`<EndMission>`, `<ObjectiveUpserted>`, `<CreateMarker>`) already cover
//! missions without it.
//!
//! # Last seen
//!
//! Counted over the maintainer's LIVE logs of 2026 (4.5 to 4.10.3). Where
//! an event's line was not found in every 4.10 build, its doc says where it
//! was last seen: an absent line may mean the game stopped writing it, or
//! only that the thing did not happen in those sessions.
//!
//! Money is `f64` because the log prints it that way
//! (`client_price[19175.000000]`) and TypeScript has one number type; ids
//! are strings (the account id outgrows nothing today, but it is an id, not
//! a quantity).

/// One event with the timestamp of the log line it came from.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub struct LogEvent {
    /// The line's own timestamp, as logged: RFC 3339, UTC, milliseconds
    /// (`2026-11-13T16:13:05.561Z`).
    pub ts: String,
    pub event: LogEventKind,
}

/// What a line said. Requests (`…Requested`) are what the client asked the
/// server for, not a confirmed outcome: the
/// [`LogEventKind::ShopFlowResponse`] that follows a shop request is its
/// answer.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(tag = "type")]
pub enum LogEventKind {
    // ── Session ─────────────────────────────────────────────────────────
    /// The game process began writing this log (its second line).
    SessionStarted,
    /// The code branch of the running build (`sc-alpha-4.10.0`).
    Branch { name: String },
    /// The changelist of the running build: the build's identity.
    Changelist { number: String },
    /// The session's environment tag: `PUB` is the live persistent
    /// universe, anything else (`PTU`, `EPTU`, …) a test environment.
    SessionPlatform { env_tag: String },
    /// The RSI handle signed in for this session.
    SessionHandle { handle: String },
    /// The numeric CIG account id of the current character. A handle rename
    /// keeps it; CIG rotated it once (Oct–Dec 2024), so it is a strong hint,
    /// not an absolute key.
    SessionAccountId {
        account_id: String,
        /// The character's GEID: what mission pushes name the player by
        /// ([`LogEventKind::MissionLeft`]).
        geid: Option<String>,
    },
    /// The player joined the persistent universe on a shard.
    ShardJoined { shard: String },
    /// The game is quitting (the player quit, or the client shut down).
    SessionEnded { reason: String },

    // ── Where ───────────────────────────────────────────────────────────
    /// The client asked for the player's inventory at a location: logged on
    /// arriving somewhere with a local inventory (a city, a station) and on
    /// opening it there. `location` is the game's internal name
    /// (`Nyx_Levski`, `RR_JP_StantonMagnus`, `Stanton4_NewBabbage`).
    LocationInventoryRequested { location: String },
    /// "Entered … Jurisdiction" on the HUD (`UEE`, `People's Alliance`,
    /// `Hurston Dynamics`, `Ungoverned`). **Locale-dependent.**
    JurisdictionEntered { jurisdiction: String },

    // ── Quantum travel ──────────────────────────────────────────────────
    // The log has no line for spooling or for the jump itself: a trip is
    // "a target was selected" and, later, "the drive arrived". The arrival
    // names no destination, so it is paired with the selection before it.
    /// The player set a quantum destination on the star map.
    QuantumTargetSelected {
        /// The class of the ship doing the routing
        /// (`RSI_Constellation_Mk5_Centaurus`): the logged entity name
        /// without its entity id. `None` when the line names no entity.
        vehicle_class: Option<String>,
        /// The destination's internal name: the entity the route ends at
        /// (`Orison_LOC`, `ObjectContainer_Lorville_City`,
        /// `rs_ext_nyx-castra_jp1`), not a star-map record name. A mission
        /// beacon's ends in its entity id.
        destination: String,
    },
    /// A route was calculated to `destination`. Logged with every
    /// [`QuantumTargetSelected`](Self::QuantumTargetSelected) (and again
    /// when the route is recalculated under way).
    QuantumRouteCalculated {
        /// Where the route starts, **as the game names it for display**
        /// (`Nyx Gateway`, `Hurston`, `CRU-L1 Ambitious Dream Station`):
        /// the one part of these lines that is a name rather than an
        /// internal id, so it most likely follows the game's language. The
        /// line around it is engine text and parses in any language.
        origin: String,
        destination: String,
    },
    /// The quantum drive arrived at the route's final destination.
    QuantumArrived { vehicle_class: Option<String> },

    // ── Gear ────────────────────────────────────────────────────────────
    /// An item arrived in one of the player's weapon or tool slots: what
    /// now sits in `port`. Only the slots an attachment can be put on
    /// through (`wep_stocked_2`, `wep_sidearm`, `weapon_attach_hand_right`,
    /// `utility_attach_1`); the log has one such line for every piece of
    /// gear, the rest are not events.
    HeldItemReceived {
        /// The item's class (`lbco_rifle_energy_01_mr01`).
        item_class: String,
        port: String,
    },
    /// The client asked to put an attachment on a weapon or tool the
    /// player carries (the inventory's equip request). The line names the
    /// attachment and the port path, not the weapon: pair `holder_port`
    /// with the last [`HeldItemReceived`](Self::HeldItemReceived) for it.
    /// **The log has no line for taking an attachment off.**
    AttachmentRequested {
        /// The attachment's class (`arma_barrel_supp_s2`).
        item_class: String,
        /// The slot the weapon sits in (`wep_stocked_2`).
        holder_port: String,
        /// The weapon's port it goes on (`barrel_attach`, `optics_attach`,
        /// `underbarrel_attach`, a multitool's `module_attach`).
        port: String,
    },

    // ── Missions ────────────────────────────────────────────────────────
    /// A contract notification on the HUD. `mission_id` is absent on the
    /// ones the game logs with a zero id (`Shared`). **Locale-dependent**
    /// ([`LogEventKind::MissionEnded`] and
    /// [`LogEventKind::MissionMarkerCreated`] are not). `Withdrawn` was last
    /// seen in 4.10.0-hotfix.
    Contract {
        change: ContractChange,
        mission_id: Option<String>,
        /// The title as shown, with Starlume's own text-patch tags removed.
        title: String,
    },
    /// An objective notification on the HUD. **Locale-dependent**
    /// ([`LogEventKind::ObjectiveUpserted`] is not).
    Objective {
        change: ObjectiveChange,
        mission_id: Option<String>,
        objective_id: Option<String>,
        text: String,
    },
    /// The server pushed an objective's state. Far more frequent than the
    /// HUD notifications, and it covers hidden objectives too.
    ObjectiveUpserted {
        mission_id: String,
        objective_id: String,
        /// `MISSION_OBJECTIVE_STATE_INPROGRESS`, `…_COMPLETED`, ….
        state: String,
    },
    /// A mission marker was created: the one line that ties a mission id to
    /// its contract (`contract`, `contract_definition_id`) and generator.
    MissionMarkerCreated {
        mission_id: String,
        generator: String,
        contract: String,
        contract_definition_id: String,
        objective_id: String,
    },
    /// A player left a mission: the server's push when you abandon one.
    /// Usually beside an `Abandon` [`LogEventKind::MissionEnded`], but
    /// sometimes the only line that says the mission is over. `player_id`
    /// is a character GEID: in a shared mission it may be someone else.
    MissionLeft {
        mission_id: String,
        player_id: String,
    },
    /// A mission ended for the player.
    MissionEnded {
        mission_id: String,
        /// `Complete`, `Fail`, `Abandon`.
        completion: String,
        /// `Mission Ended`, `Player left`.
        reason: String,
    },

    // ── Economy ─────────────────────────────────────────────────────────
    /// "Awarded N aUEC" / "Fined N UEC" on the HUD. **Locale-dependent.**
    /// Both last seen in 4.10.0; none in the 4.10.0-hotfix to 4.10.3 logs.
    Money { change: MoneyChange, amount: f64 },
    /// The client asked a commodity kiosk to buy or sell.
    ///
    /// **Not seen in any 4.10 log**: a buy was last seen in 4.9.0, a sell
    /// in 4.8.0-hotfix. Unconfirmed for the current game until one shows
    /// up.
    CommodityTradeRequested {
        side: TradeSide,
        /// The shop's internal name (`SCShop_Levski_CargoOffice_Commodities`).
        shop: String,
        /// The commodity's resource record guid.
        resource_guid: String,
        /// Total price of the request, in aUEC.
        price: f64,
        quantity: f64,
        /// The unit printed with the quantity (`cSCU` on a buy); absent
        /// when the log prints a bare number (a sell).
        quantity_unit: Option<String>,
    },
    /// The client asked a shop to sell the player an item (a kiosk
    /// purchase, or a ship from a dealer). Last seen in 4.10.2; none in the
    /// one 4.10.3 log (nothing was bought in it).
    ShopBuyRequested {
        shop: String,
        item_guid: String,
        /// The item's class name (`grin_tractor_01`, `ARGO_MOTH`).
        item_name: String,
        /// Total price as the client computed it, in aUEC.
        price: f64,
        quantity: u32,
    },
    /// The client asked a shop to rent the player an item (a ship rental
    /// kiosk). Seen in 4.10.3.
    ShopRentalRequested {
        shop: String,
        item_guid: String,
        /// The item's class name (`RSI_Constellation_Andromeda_GS`).
        item_name: String,
        /// As the client computed it, in aUEC (`0` at a free event rental).
        price: f64,
    },
    /// The server answered the last shop request: a
    /// [`ShopBuyRequested`](Self::ShopBuyRequested), a
    /// [`ShopRentalRequested`](Self::ShopRentalRequested) or a
    /// [`CommodityTradeRequested`](Self::CommodityTradeRequested). The line
    /// does not say which, so pair it with the request just before it.
    ShopFlowResponse {
        success: bool,
        /// `Success`, `AuthorityError`, ….
        result: String,
    },
    /// "A Refinery Work Order has been Completed at …" on the HUD.
    /// **Locale-dependent.** Last seen in 4.10.2; none in the one 4.10.3
    /// log.
    RefineryOrderCompleted { location: String },
    /// "Received Blueprint: …" on the HUD. The display name only: the log
    /// never carries the record guid. **Locale-dependent** (and the name is
    /// the localized one).
    BlueprintReceived { name: String },

    // ── Combat ──────────────────────────────────────────────────────────
    /// A vehicle was destroyed near the player: one they shot down, one of
    /// their own, or anyone's the client saw go. Logged once per step
    /// (`level [0] -> [2]`; a ship disabled first may log a second line for
    /// the same `vehicle_id`). Seen in 4.10.
    VehicleDestroyed {
        /// The vehicle's entity id.
        vehicle_id: String,
        /// Its class (`DRAK_Marauder`, `MISC_Razor_LX_PU_AI_CRIM`).
        class: String,
        /// The GEID of the player who owns it; `None` for an NPC's (`[0]`).
        owner: Option<String>,
        /// `Combat`, `Collision`, `SelfDestruct`, `ShipItemCritical`, ….
        reason: String,
        /// The destruction level reached: 1 disabled, 2 destroyed.
        level: u8,
        /// Who or what destroyed it: a handle, an NPC's entity name, or the
        /// vehicle itself (a self-destruct).
        killer: String,
        /// The killer's GEID or entity id.
        killer_id: String,
    },

    // ── Health ──────────────────────────────────────────────────────────
    /// "Minor Injury Detected - Left arm - Tier 3 Treatment Required" on
    /// the HUD. The tier is the med bed it needs: 3 minor, 2 moderate,
    /// 1 severe. **Locale-dependent.** Seen in 4.0 to 4.10.
    Injured { part: BodyPart, tier: u8 },
    /// A med bed operated (`<MED BED HEAL>`). The client logs other
    /// players' surgery too, so `actor` says whose. Only `Success` seen in
    /// 4.0 to 4.10.
    MedBedSurgery {
        /// The patient's handle.
        actor: String,
        /// `Success`.
        outcome: String,
        /// The ship the bed is in, as its class (`RSI_Polaris`); `None` for
        /// a bed in a station or hospital.
        vehicle: Option<String>,
        /// The parts operated on.
        parts: Vec<BodyPart>,
    },

    // ── Vehicle terminal ────────────────────────────────────────────────
    /// The vehicle terminal (ASOP) asked for your ships: it was opened or
    /// refreshed (also once while the character loads). The log has no
    /// line for what is paid there (repair, refuel, restock, claim), so
    /// this is the only trace of it. Seen in 4.10.
    VehicleListFetched,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum ContractChange {
    Available,
    Accepted,
    Shared,
    Completed,
    Failed,
    Withdrawn,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum ObjectiveChange {
    New,
    Completed,
    Withdrawn,
}

/// A body part, as injuries and med beds name them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum BodyPart {
    Head,
    Torso,
    LeftArm,
    RightArm,
    LeftLeg,
    RightLeg,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum MoneyChange {
    Awarded,
    Fined,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
pub enum TradeSide {
    Buy,
    Sell,
}
