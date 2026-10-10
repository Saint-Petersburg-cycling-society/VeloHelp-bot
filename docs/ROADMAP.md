# Roadmap / Дорожная карта

## RU

### Phase 0
- [x] AGPL-3.0-only
- [x] НКО/organizational context
- [x] frontend/backend boundary
- [x] SQLite ADR + migration
- [x] bilingual docs policy
- [x] custom icon system contract
- [x] deployment examples
- [ ] final repo URL/name
- [ ] production BotFather bot
- [ ] operator/legal review

### Phase 1 — backend/domain
- [ ] SQLite repositories
- [ ] helper profile
- [ ] area resolver
- [ ] create/cancel/expire
- [ ] matching
- [ ] atomic accept
- [ ] concurrent accept integration test

### Phase 2 — frontend
- [ ] `/start`
- [ ] consent/legal
- [ ] reusable MultiSelect/Wizard/Confirm
- [ ] location external input
- [ ] dispatch cards
- [ ] matched surfaces
- [ ] custom icon pack v1

### Phase 3 — upstream teloxide-ui
- [ ] callback dispatcher + ACK
- [ ] per-action policy
- [ ] external input primitive
- [ ] multi-surface effect executor
- [ ] persistence/recovery
- [ ] cross-view invalidation
- [ ] reusable components

### Phase 4 — operations
- [ ] TTL worker
- [ ] rate limits
- [ ] metrics
- [ ] backup timer + restore test
- [ ] deletion/export
- [ ] source/legal UI
- [ ] deployment hardening

## EN

Phase 0 establishes licensing, NGO context, architecture, SQLite, bilingual
documentation, icon and deployment contracts. Phase 1 implements authoritative
domain/storage behavior. Phase 2 builds the Telegram frontend from reusable
components. Phase 3 extracts proven generic mechanics into MIT `teloxide-ui`.
Phase 4 hardens operations, retention, recovery and deployment.
