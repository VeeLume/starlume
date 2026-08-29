<!-- Contract Tracker — skeleton page. The module is registered (picker,
     sidebar, settings) but has no features yet; this page states honestly
     what's planned so an enabled module never dead-ends on a blank screen.
     Replace section by section as features land (tracker → rep filter →
     routes → cargo packing). -->

<h1>Contracts</h1>
<p class="intro dim">
  The contract tracker is under construction. It lands in stages — everything below is
  planned, nothing is functional yet.
</p>

<div class="card">
  <h2>Coming here</h2>
  <ul class="planned">
    <li>
      <strong>Active contracts</strong> — every accepted contract with per-objective
      progress, payout, and reputation at a glance.
    </li>
    <li>
      <strong>Eligibility</strong> — see which offered contracts you actually unlock, and
      how far off the rest are (reputation-gate matching against your own standings).
    </li>
    <li>
      <strong>Route planning</strong> — one route across all running contracts, not per
      contract.
    </li>
    <li>
      <strong>Cargo packing</strong> — the hauling view: manifest legs, box sizes, grid
      fits.
    </li>
  </ul>
  <p class="dim note">
    Contract data comes from CIG game services and will require the game-services opt-in
    (Settings → Online) once fetching lands.
  </p>
</div>

<style>
  .intro {
    max-width: 60ch;
  }
  .planned {
    margin: 0;
    padding-left: 1.1rem;
    display: grid;
    gap: 6px;
  }
  .note {
    margin-bottom: 0;
  }
</style>
