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

Bootstrap pin:

```text
https://github.com/Mar2ianen/teloxide-ui
70da4d21c327d70e18425eb97503e6b97e94a30e
```

The bot MUST pull only generic mechanics into the library:

1. callback dispatcher + automatic ACK;
2. per-action actor/stale/TTL policy;
3. external input events (location/text) that never pretend to be callbacks;
4. `RenderAt` / `Send` / `Delete` / toast / alert;
5. persisted/recoverable UI runtime;
6. invalidation capabilities across multiple `ViewId`;
7. reusable `MultiSelect`, `Wizard`, `Confirm`, `ActionBar`, `StatusBadge`;
8. stable icon/custom-emoji component contract.

### Extraction rule

```text
real pain in bike-aid
→ minimal app implementation
→ isolate the generic invariant
→ clean MIT primitive in teloxide-ui
→ migrate bike-aid to the primitive
```

Generic APIs MUST NOT contain Bike/Bicycle-specific types.
