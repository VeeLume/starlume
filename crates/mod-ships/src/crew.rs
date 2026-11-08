//! Who a ship needs: its seats by what their occupant controls
//! (sc-vehicles' `Crew`, from the seats' control priorities, not their
//! names). A seat that controls nothing is a passenger's.

use serde::{Deserialize, Serialize};
use svc_data::CookedData;
use svc_data::ships::{Crew, Mounted, Role, Vehicle};

/// A loadout's seats by role. A seat can hold several roles (a gunner seat
/// that also mines), so the roles can add up to more than [`Seats::crew`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Seats {
    pub pilot: usize,
    /// Can take the helm below the pilot.
    pub copilot: usize,
    /// Seats in a turret.
    pub manned_turrets: usize,
    /// Seats that aim turrets they do not sit in.
    pub remote_turrets: usize,
    pub mining: usize,
    pub salvage: usize,
    /// Seats with any role.
    pub crew: usize,
    /// Seats with none.
    pub passengers: usize,
}

/// The seats of `mounted` on `vehicle`.
pub fn seats(cooked: &CookedData, vehicle: &Vehicle, mounted: &[Mounted]) -> Seats {
    let index = &cooked.ships;
    let Some(items) = cooked.holotable.items.as_ref() else {
        return Seats::default();
    };
    let crew = Crew::compute(vehicle, mounted, &index.vehicles, items, &index.containers);
    let size = crew.size();
    Seats {
        pilot: crew.count(&Role::Pilot),
        copilot: crew.count(&Role::Copilot),
        manned_turrets: crew.count(&Role::MannedTurret),
        remote_turrets: crew.count(&Role::RemoteTurret {
            turrets: Vec::new(),
        }),
        mining: crew.count(&Role::Mining),
        salvage: crew.count(&Role::Salvage),
        crew: size,
        passengers: crew.stations.len() - size,
    }
}
