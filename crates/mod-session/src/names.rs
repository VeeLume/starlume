//! Fallback names: the game's internal names made readable.
//!
//! A session stores what the log and the stream said: `Stanton4_NewBabbage`,
//! `SCShop_CousinCrows_Orison`, a vehicle's class name. The shell resolves
//! those against the reference catalogs (`session_names`); what a catalog
//! does not know is shown through these cleaners instead of raw. They only
//! reshape the text: nothing here knows a place or a shop.

/// Split an internal name into words: `_`, `-` and spaces separate, and so
/// does a camel-case boundary (`NewBabbage`, `HDShowcase`).
fn words(raw: &str) -> Vec<String> {
    let mut out = Vec::new();
    for token in raw.split(['_', '-', ' ']).filter(|t| !t.is_empty()) {
        let chars: Vec<char> = token.chars().collect();
        let mut word = String::new();
        for (i, &c) in chars.iter().enumerate() {
            let boundary = i > 0 && c.is_uppercase() && {
                let prev = chars[i - 1];
                let next_lower = chars.get(i + 1).is_some_and(|n| n.is_lowercase());
                prev.is_lowercase() || (prev.is_uppercase() && next_lower)
            };
            if boundary {
                out.push(std::mem::take(&mut word));
            }
            word.push(c);
        }
        out.push(word);
    }
    out
}

fn is_number(word: &str) -> bool {
    !word.is_empty() && word.chars().all(|c| c.is_ascii_digit())
}

/// The words without a trailing instance counter (`-001`).
fn without_counter(mut words: Vec<String>) -> Vec<String> {
    while words.len() > 1 && words.last().is_some_and(|w| is_number(w)) {
        words.pop();
    }
    words
}

/// A place by its internal name, when no catalog knows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CleanPlace {
    pub name: String,
    /// The system, when the name leads with one (`Stanton4_…`, `Nyx_…`).
    pub system: Option<String>,
}

/// `Stanton4_NewBabbage` → New Babbage (Stanton); `RR_JP_NyxCastra` → Nyx
/// Castra jump point.
pub fn clean_place(internal: &str) -> CleanPlace {
    let tokens: Vec<&str> = internal.split('_').filter(|t| !t.is_empty()).collect();
    // Rest stops lead with `RR`; a jump point's is `RR_JP_<A><B>`.
    let tokens = match tokens.as_slice() {
        ["RR", rest @ ..] if !rest.is_empty() => rest,
        all => all,
    };
    if let ["JP", rest @ ..] = tokens
        && !rest.is_empty()
    {
        return CleanPlace {
            name: format!("{} jump point", words(&rest.join("_")).join(" ")),
            system: None,
        };
    }
    if let [first, rest @ ..] = tokens
        && !rest.is_empty()
    {
        let system = first.trim_end_matches(|c: char| c.is_ascii_digit());
        let leads_with_system = !system.is_empty()
            && system.chars().next().is_some_and(char::is_uppercase)
            && system.chars().skip(1).all(char::is_lowercase);
        if leads_with_system {
            return CleanPlace {
                name: without_counter(words(&rest.join("_"))).join(" "),
                system: Some(system.to_owned()),
            };
        }
    }
    CleanPlace {
        name: without_counter(words(&tokens.join("_"))).join(" "),
        system: None,
    }
}

/// The last `_` token of a place's internal name: what a shop's name ends
/// with when it carries its place (`Stanton2_Orison` → `Orison`).
fn place_token(place: &str) -> Option<&str> {
    place.rsplit('_').find(|t| !t.is_empty())
}

/// `SCShop_CousinCrows_Orison` → Cousin Crows. `places` are the internal
/// names of the places the session visited: a shop name that ends with one
/// of them loses it (the place is shown beside the shop anyway).
pub fn clean_shop(internal: &str, places: &[String]) -> String {
    let body = internal
        .strip_prefix("SCShop_")
        .or_else(|| internal.strip_prefix("Shop_"))
        .unwrap_or(internal);
    let mut tokens: Vec<&str> = body.split(['_', '-']).filter(|t| !t.is_empty()).collect();
    while tokens.len() > 1 && tokens.last().is_some_and(|t| is_number(t)) {
        tokens.pop();
    }
    if tokens.len() > 1
        && let Some(last) = tokens.last()
        && places
            .iter()
            .filter_map(|p| place_token(p))
            .any(|p| p.eq_ignore_ascii_case(last))
    {
        tokens.pop();
    }
    words(&tokens.join("_")).join(" ")
}

/// `RSI_Constellation_Mk5_Centaurus` → RSI Constellation Mk5 Centaurus.
pub fn clean_vehicle(class_name: &str) -> String {
    words(class_name).join(" ")
}

/// `HRST_LaserRepeater_S5` → HRST Laser Repeater S5. The entity suffix
/// (`_SCItem`) says nothing and goes.
pub fn clean_item(class_name: &str) -> String {
    let body = class_name
        .strip_suffix("_SCItem")
        .or_else(|| class_name.strip_suffix("_scitem"))
        .unwrap_or(class_name);
    words(body).join(" ")
}

/// A contract's internal name (`InterSec_BoardShip`), for a mission whose
/// HUD title was never seen.
pub fn clean_contract(internal: &str) -> String {
    words(internal).join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn words_split_on_separators_and_camel_case() {
        assert_eq!(words("NewBabbage"), ["New", "Babbage"]);
        assert_eq!(
            words("ShipWeapon_HDShowcase"),
            ["Ship", "Weapon", "HD", "Showcase"]
        );
        assert_eq!(words("lbco_rifle_energy_01_mag").len(), 5);
        assert_eq!(words("Mk5"), ["Mk5"]);
        assert!(words("").is_empty());
    }

    #[test]
    fn a_place_leads_with_its_system() {
        assert_eq!(
            clean_place("Stanton4_NewBabbage"),
            CleanPlace {
                name: "New Babbage".into(),
                system: Some("Stanton".into()),
            }
        );
        assert_eq!(
            clean_place("Nyx_Levski"),
            CleanPlace {
                name: "Levski".into(),
                system: Some("Nyx".into()),
            }
        );
        // One word: nothing to split a system from.
        assert_eq!(clean_place("Levski").system, None);
        assert_eq!(clean_place("Levski").name, "Levski");
    }

    #[test]
    fn a_jump_point_says_so() {
        let jp = clean_place("RR_JP_NyxCastra");
        assert_eq!(jp.name, "Nyx Castra jump point");
        assert_eq!(jp.system, None);
        // A rest stop keeps its own name.
        assert_eq!(clean_place("RR_HUR_L1").name, "HUR L1");
    }

    #[test]
    fn a_shop_loses_its_prefix_its_counter_and_its_place() {
        let places = ["Stanton2_Orison".to_owned(), "Nyx_Levski".to_owned()];
        assert_eq!(
            clean_shop("SCShop_CousinCrows_Orison", &places),
            "Cousin Crows"
        );
        assert_eq!(
            clean_shop("SCShop_ConscientiousObjects_Levski-001", &places),
            "Conscientious Objects"
        );
        // A place the session never named stays in the shop's name.
        assert_eq!(
            clean_shop("SCShop_ShipWeapon_HDShowcase_Lorville", &places),
            "Ship Weapon HD Showcase Lorville"
        );
        // Never cleaned down to nothing.
        assert_eq!(clean_shop("SCShop_Orison", &places), "Orison");
    }

    #[test]
    fn vehicles_items_and_contracts_read_as_words() {
        assert_eq!(
            clean_vehicle("RSI_Constellation_Mk5_Centaurus"),
            "RSI Constellation Mk5 Centaurus"
        );
        assert_eq!(clean_item("SHLD_BEHR_S03_7CA_SCItem"), "SHLD BEHR S03 7CA");
        assert_eq!(
            clean_item("HRST_LaserRepeater_S5"),
            "HRST Laser Repeater S5"
        );
        assert_eq!(clean_contract("InterSec_BoardShip"), "Inter Sec Board Ship");
    }
}
