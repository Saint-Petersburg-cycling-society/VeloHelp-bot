# teloxide-ui dogfood / План dogfood

## RU

Bootstrap pin:

```text
https://github.com/Mar2ianen/teloxide-ui
70da4d21c327d70e18425eb97503e6b97e94a30e
```

Бот должен вытянуть в библиотеку только общие механики:

1. callback dispatcher + automatic ACK;
2. per-action actor/stale/TTL policy;
3. external input events (location/text), не притворяющиеся callback;
4. `RenderAt` / `Send` / `Delete` / toast / alert;
5. persisted/recoverable UI runtime;
6. invalidation capabilities между несколькими `ViewId`;
7. reusable `MultiSelect`, `Wizard`, `Confirm`, `ActionBar`, `StatusBadge`;
8. стабильный icon/custom-emoji component contract.

### Правило extraction

```text
реальная боль в bike-aid
→ минимальная app implementation
→ выделение общего invariant
→ чистый MIT primitive в teloxide-ui
→ миграция bike-aid на primitive
```

Generic API не должен содержать Bike/Bicycle-specific types.

## EN

This application exists to force real reusable primitives out of
`teloxide-ui`: callback dispatch/ACK, per-action policies, external input,
multi-surface effects, persistence/recovery, cross-view invalidation and
reusable form/navigation components.

The extraction loop is: observe real application pain, implement minimally,
identify the invariant, implement a clean generic MIT primitive upstream, then
migrate the AGPL application to it. Generic APIs must not contain bicycle
domain concepts.
