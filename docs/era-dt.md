# Era → dt mapping (stub — numbers to be finalized with science reviewer + perf actuals)
# Coupler phases per era: [tectonics dt] → [climate subcycle] → [geochem] → [bio/eco] → [audit]

| era | sync_yr | climate subcycle | tectonics dt | notes |
|-----|---------|------------------|--------------|-------|
| Hadean | 1e6 | EMBM annual (no dyn) | 1 Myr steps ok | magma ocean → solidification |
| Archean | 1e5 | EMBM annual | 100 kyr | faint young sun; low O2 |
| Proterozoic | 1e4 | EMBM + daily bursts | 10 kyr | GOE, Snowball windows |
| Phanerozoic | 1e3 | daily dynamics-lite | 1 kyr | default deep-time |
| Quaternary/twin | 1 | daily full | 1 yr (prescribed ice) | Milankovitch + GHG prescribed |
| Twin forecast | 1 | daily full | 1 yr | ensembles IC/PARAM/STOCH |

Conservation rel-tols (stub, arch C5/P5): carbon 1e-6/tick, water 1e-6, energy 1e-5; retry-halve-dt once then abort.
