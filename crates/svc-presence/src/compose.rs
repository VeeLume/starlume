//! A moment of play, and what the profile shows of it (decision 01a2085d,
//! board "Discord Rich Presence" on the Starlume Design canvas):
//!
//! - **Details** says what: "On foot", "Flying a Galaxy", or a contract as
//!   its type and who gives it ("Mercenary for Headhunters").
//! - **State** says where, at the level the user shares: the exact place,
//!   only the system, or nothing, and then the contract's progress instead.
//! - **The large image** is the ship: its store picture by URL, its name on
//!   hover. No ship: the Starlume mark.
//! - **The small image** is what you are doing, as an icon; its hover holds
//!   the contract (title, objective, progress) or the exact place.
//! - **The timer** counts from the contract's start while one shows, else
//!   from the session's start (Discord shows elapsed time anyway; sending
//!   the start keeps it right across a Starlume restart).
//!
//! The caller resolves every name (catalogs, the ship store); this module
//! only picks and shortens. A [`Moment`] reads from JSON, so the `presence`
//! example can send hand-written ones.

use serde::{Deserialize, Serialize};

use crate::activity::{Activity, Assets, Button, LINE_MAX, Timestamps, cut};

/// Discord's limit for a hover text.
const HOVER_MAX: usize = 128;

/// The asset keys uploaded to the Discord application: the mark, and one
/// icon per thing you can be doing.
pub mod asset {
    pub const MARK: &str = "starlume";
    pub const LOADING: &str = "loading";
    pub const ON_FOOT: &str = "on_foot";
    pub const FLYING: &str = "flying";
    pub const QUANTUM: &str = "quantum";
    pub const CONTRACT: &str = "contract";
    pub const MINING: &str = "mining";
}

/// Where the "Get Starlume" button goes.
pub const STARLUME_URL: &str = "https://github.com/VeeLume/starlume/releases/latest";

/// What the user lets the profile show (Settings › Discord).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Share {
    pub place: PlaceLevel,
    /// The ship's picture and name.
    pub ship: bool,
    /// The contract's type and faction, and its title and progress on
    /// hover.
    pub contracts: bool,
    /// The "Get Starlume" button.
    pub button: bool,
}

impl Default for Share {
    fn default() -> Self {
        Share {
            place: PlaceLevel::System,
            ship: true,
            contracts: true,
            button: false,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlaceLevel {
    /// The station, moon or quantum target.
    Exact,
    /// Only the system: Stanton, Pyro, Nyx.
    #[default]
    System,
    /// No place; State shows progress.
    Off,
}

/// One moment of play, names resolved.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Moment {
    pub doing: Doing,
    /// Where you are; in quantum travel, where you are going.
    pub place: Option<Place>,
    /// The ship you are in.
    pub ship: Option<Ship>,
    /// The contract to show: the newest one still running.
    pub contract: Option<Contract>,
    /// When the game session started, Unix seconds.
    pub session_start: Option<i64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Doing {
    /// The game runs, no shard joined yet.
    #[default]
    Loading,
    OnFoot,
    Flying,
    Quantum,
    Mining,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Place {
    /// "Levski, Nyx", "Yela", "Onyx Facility, Lyria".
    pub exact: String,
    /// "Nyx", "Stanton".
    pub system: String,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ship {
    /// The full name, for the hover: "RSI Galaxy".
    pub name: String,
    /// The name in a sentence: "Galaxy".
    pub short: String,
    /// Its ship store picture, an https URL.
    pub picture: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Contract {
    /// The catalog's type: "Mercenary", "Hauling - Stellar".
    pub kind: Option<String>,
    /// Who gives it: "Headhunters".
    pub faction: Option<String>,
    /// As the game titles it, without our tags.
    pub title: String,
    /// The current objective. It can name a place ("Go to Onyx Facility
    /// S3A4"), so it only goes out when the exact place does.
    pub objective: Option<String>,
    /// Progress that names no place: "7 contracts done".
    pub progress: Option<String>,
    /// When it was taken, Unix seconds.
    pub started: Option<i64>,
}

/// What the profile shows of `moment`, as `share` allows.
pub fn compose(moment: &Moment, share: &Share) -> Activity {
    let contract = moment.contract.as_ref().filter(|_| share.contracts);
    let ship = moment.ship.as_ref().filter(|_| share.ship);
    let exact = share.place == PlaceLevel::Exact;

    let details = match contract {
        Some(c) => contract_line(c),
        None => doing_line(moment.doing, ship),
    };

    let state = match (share.place, &moment.place) {
        (PlaceLevel::Off, _) => contract.and_then(|c| c.progress.clone()),
        (_, None) => None,
        (PlaceLevel::Exact, Some(p)) if moment.doing == Doing::Quantum => {
            Some(format!("To {}", p.exact))
        }
        (PlaceLevel::Exact, Some(p)) => Some(p.exact.clone()),
        (PlaceLevel::System, Some(p)) => Some(p.system.clone()),
    };

    let (large_image, large_text) = match ship {
        Some(s) => (
            s.picture.clone().unwrap_or_else(|| asset::MARK.into()),
            s.name.clone(),
        ),
        None => (asset::MARK.into(), "Starlume".into()),
    };

    let small_image = match (contract, moment.doing) {
        (Some(_), _) => asset::CONTRACT,
        (None, Doing::Loading) => asset::LOADING,
        (None, Doing::OnFoot) => asset::ON_FOOT,
        (None, Doing::Flying) => asset::FLYING,
        (None, Doing::Quantum) => asset::QUANTUM,
        (None, Doing::Mining) => asset::MINING,
    };
    let small_text = match contract {
        Some(c) => {
            let objective = c.objective.as_deref().filter(|_| exact);
            let parts: Vec<&str> = [Some(c.title.as_str()), objective, c.progress.as_deref()]
                .into_iter()
                .flatten()
                .filter(|s| !s.trim().is_empty())
                .collect();
            Some(parts.join(", "))
        }
        None => moment
            .place
            .as_ref()
            .filter(|_| exact)
            .map(|p| p.exact.clone()),
    };

    let start = contract.and_then(|c| c.started).or(moment.session_start);

    Activity {
        details: text(&details, LINE_MAX),
        state: state.and_then(|s| text(&s, LINE_MAX)),
        timestamps: start.map(|start| Timestamps {
            start: Some(start),
            end: None,
        }),
        assets: Some(Assets {
            large_image: Some(large_image),
            large_text: text(&large_text, HOVER_MAX),
            small_image: Some(small_image.into()),
            small_text: small_text.and_then(|s| text(&s, HOVER_MAX)),
        }),
        buttons: if share.button {
            vec![Button {
                label: "Get Starlume".into(),
                url: STARLUME_URL.into(),
            }]
        } else {
            Vec::new()
        },
    }
}

/// A contract's top line: its type and who gives it, or only the type when
/// both don't fit, or "On a contract" when the catalog knows neither.
fn contract_line(c: &Contract) -> String {
    let kind = c
        .kind
        .as_deref()
        .map(|k| k.split(" - ").next().unwrap_or(k).trim())
        .filter(|k| !k.is_empty());
    match (kind, c.faction.as_deref().filter(|f| !f.trim().is_empty())) {
        (Some(k), Some(f)) if k.chars().count() + f.chars().count() + 5 <= LINE_MAX => {
            format!("{k} for {f}")
        }
        (Some(k), _) => k.to_owned(),
        (None, _) => "On a contract".into(),
    }
}

fn doing_line(doing: Doing, ship: Option<&Ship>) -> String {
    match doing {
        Doing::Loading => "Loading in".into(),
        Doing::OnFoot => "On foot".into(),
        Doing::Mining => "Mining".into(),
        Doing::Flying | Doing::Quantum => {
            match ship.map(|s| s.short.trim()).filter(|s| !s.is_empty()) {
                Some(s) => format!("Flying {} {s}", article(s)),
                None => "Flying".into(),
            }
        }
    }
}

fn article(word: &str) -> &'static str {
    match word.chars().next().map(|c| c.to_ascii_lowercase()) {
        Some('a' | 'e' | 'i' | 'o' | 'u') => "an",
        _ => "a",
    }
}

/// A text Discord accepts: cut to `max`, and at least two characters
/// (Discord refuses the whole activity over a shorter one).
fn text(s: &str, max: usize) -> Option<String> {
    let s = cut(s, max);
    (s.chars().count() >= 2).then_some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn place(exact: &str, system: &str) -> Option<Place> {
        Some(Place {
            exact: exact.into(),
            system: system.into(),
        })
    }

    fn ship(name: &str, short: &str, picture: Option<&str>) -> Option<Ship> {
        Some(Ship {
            name: name.into(),
            short: short.into(),
            picture: picture.map(Into::into),
        })
    }

    const GALAXY: &str = "https://media.robertsspaceindustries.com/bb6z60h2ou5a0/slideshow.jpg";

    /// Valerie's Nov 22 session at 3:48: the Jorrit Dossier in her Hermes.
    fn dossier() -> Moment {
        Moment {
            doing: Doing::OnFoot,
            place: place("Onyx Facility, Lyria", "Stanton"),
            ship: ship("RSI Hermes", "Hermes", None),
            contract: Some(Contract {
                kind: Some("Investigation".into()),
                faction: None,
                title: "Jorrit Dossier: Retrieve Security Data".into(),
                objective: Some("Retrieve Security Data from Research Wing".into()),
                progress: Some("1 objective done".into()),
                started: Some(1_795_389_000),
            }),
            session_start: Some(1_795_375_494),
        }
    }

    /// The Nov 22 session at 1:13: the Galaxy in quantum to Yela.
    fn galaxy() -> Moment {
        Moment {
            doing: Doing::Quantum,
            place: place("Yela", "Stanton"),
            ship: ship("RSI Galaxy", "Galaxy", Some(GALAXY)),
            contract: None,
            session_start: Some(1_795_375_494),
        }
    }

    fn assets(a: &Activity) -> &Assets {
        a.assets.as_ref().unwrap()
    }

    #[test]
    fn in_quantum_details_names_the_ship_and_state_the_target() {
        let a = compose(
            &galaxy(),
            &Share {
                place: PlaceLevel::Exact,
                ..Share::default()
            },
        );
        assert_eq!(a.details.as_deref(), Some("Flying a Galaxy"));
        assert_eq!(a.state.as_deref(), Some("To Yela"));
        assert_eq!(assets(&a).large_image.as_deref(), Some(GALAXY));
        assert_eq!(assets(&a).large_text.as_deref(), Some("RSI Galaxy"));
        assert_eq!(assets(&a).small_image.as_deref(), Some(asset::QUANTUM));
        assert_eq!(assets(&a).small_text.as_deref(), Some("Yela"));
    }

    #[test]
    fn the_system_level_says_only_the_system_and_hovers_nothing() {
        let a = compose(&galaxy(), &Share::default());
        assert_eq!(a.state.as_deref(), Some("Stanton"));
        assert_eq!(assets(&a).small_text, None);
    }

    #[test]
    fn a_contract_is_its_type_and_the_timer_starts_with_it() {
        let a = compose(&dossier(), &Share::default());
        assert_eq!(a.details.as_deref(), Some("Investigation"));
        assert_eq!(a.state.as_deref(), Some("Stanton"));
        assert_eq!(a.timestamps.as_ref().unwrap().start, Some(1_795_389_000));
        assert_eq!(assets(&a).small_image.as_deref(), Some(asset::CONTRACT));
        // No objective below the exact level: it names a place.
        assert_eq!(
            assets(&a).small_text.as_deref(),
            Some("Jorrit Dossier: Retrieve Security Data, 1 objective done")
        );
    }

    #[test]
    fn the_exact_level_adds_the_objective_to_the_hover() {
        let a = compose(
            &dossier(),
            &Share {
                place: PlaceLevel::Exact,
                ..Share::default()
            },
        );
        assert_eq!(a.state.as_deref(), Some("Onyx Facility, Lyria"));
        assert_eq!(
            assets(&a).small_text.as_deref(),
            Some(
                "Jorrit Dossier: Retrieve Security Data, Retrieve Security Data from Research Wing, 1 objective done"
            )
        );
    }

    #[test]
    fn with_the_place_off_state_shows_progress() {
        let a = compose(
            &dossier(),
            &Share {
                place: PlaceLevel::Off,
                ..Share::default()
            },
        );
        assert_eq!(a.state.as_deref(), Some("1 objective done"));
        let a = compose(
            &galaxy(),
            &Share {
                place: PlaceLevel::Off,
                ..Share::default()
            },
        );
        assert_eq!(a.state, None);
    }

    #[test]
    fn contracts_off_falls_back_to_the_ship_and_the_session_timer() {
        let mut m = dossier();
        m.doing = Doing::Flying;
        let a = compose(
            &m,
            &Share {
                contracts: false,
                ..Share::default()
            },
        );
        assert_eq!(a.details.as_deref(), Some("Flying a Hermes"));
        assert_eq!(a.timestamps.as_ref().unwrap().start, Some(1_795_375_494));
        assert_eq!(assets(&a).small_image.as_deref(), Some(asset::FLYING));
    }

    #[test]
    fn ship_off_hides_its_picture_and_its_name() {
        let a = compose(
            &galaxy(),
            &Share {
                ship: false,
                ..Share::default()
            },
        );
        assert_eq!(a.details.as_deref(), Some("Flying"));
        assert_eq!(assets(&a).large_image.as_deref(), Some(asset::MARK));
        assert_eq!(assets(&a).large_text.as_deref(), Some("Starlume"));
    }

    #[test]
    fn a_ship_without_a_store_picture_keeps_the_mark_and_its_name() {
        let m = Moment {
            doing: Doing::Flying,
            ship: ship("Kruger Stingray", "Stingray", None),
            ..Moment::default()
        };
        let a = compose(&m, &Share::default());
        assert_eq!(assets(&a).large_image.as_deref(), Some(asset::MARK));
        assert_eq!(assets(&a).large_text.as_deref(), Some("Kruger Stingray"));
    }

    #[test]
    fn type_and_faction_drop_the_faction_when_they_do_not_fit() {
        let c = |kind: &str, faction: &str| Contract {
            kind: Some(kind.into()),
            faction: Some(faction.into()),
            ..Contract::default()
        };
        assert_eq!(
            contract_line(&c("Mercenary", "Headhunters")),
            "Mercenary for Headhunters"
        );
        assert_eq!(
            contract_line(&c("Hauling - Stellar", "Covalex")),
            "Hauling for Covalex"
        );
        assert_eq!(
            contract_line(&c("Mercenary", "Citizens For Prosperity")),
            "Mercenary"
        );
        assert_eq!(contract_line(&Contract::default()), "On a contract");
    }

    #[test]
    fn an_before_a_vowel() {
        let m = Moment {
            doing: Doing::Flying,
            ship: ship("Aegis Avenger Titan", "Avenger Titan", None),
            ..Moment::default()
        };
        assert_eq!(
            compose(&m, &Share::default()).details.as_deref(),
            Some("Flying an Avenger Titan")
        );
    }

    #[test]
    fn loading_shows_the_mark_and_no_place() {
        let m = Moment {
            session_start: Some(10),
            ..Moment::default()
        };
        let a = compose(
            &m,
            &Share {
                place: PlaceLevel::Exact,
                ..Share::default()
            },
        );
        assert_eq!(a.details.as_deref(), Some("Loading in"));
        assert_eq!(a.state, None);
        assert_eq!(assets(&a).small_image.as_deref(), Some(asset::LOADING));
        assert_eq!(a.timestamps.unwrap().start, Some(10));
    }

    #[test]
    fn the_button_only_when_shared() {
        assert!(compose(&galaxy(), &Share::default()).buttons.is_empty());
        let a = compose(
            &galaxy(),
            &Share {
                button: true,
                ..Share::default()
            },
        );
        assert_eq!(a.buttons[0].url, STARLUME_URL);
    }

    #[test]
    fn a_moment_reads_from_sparse_json() {
        let m: Moment = serde_json::from_str(
            r#"{"doing":"on_foot","place":{"exact":"Levski, Nyx","system":"Nyx"}}"#,
        )
        .unwrap();
        let a = compose(
            &m,
            &Share {
                place: PlaceLevel::Exact,
                ..Share::default()
            },
        );
        assert_eq!(a.details.as_deref(), Some("On foot"));
        assert_eq!(a.state.as_deref(), Some("Levski, Nyx"));
        assert_eq!(a.timestamps, None);
    }
}
