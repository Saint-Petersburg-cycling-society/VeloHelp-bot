# State machines / Машины состояний

## RU

### HelpRequest

```mermaid
stateDiagram-v2
    [*] --> Draft
    Draft --> Open: publish
    Draft --> Cancelled: cancel
    Open --> Matched: first atomic accept
    Open --> Cancelled: requester cancels
    Open --> Expired: TTL
    Matched --> Resolved: requester confirms
    Matched --> Open: helper withdraws
    Matched --> Cancelled: requester cancels
```

`Matched -> Open` создаёт новый request version / новую dispatch wave.
Старые capabilities не воскресают.

### Dispatch

```mermaid
stateDiagram-v2
    [*] --> Pending
    Pending --> Accepted
    Pending --> Declined
    Pending --> Superseded
    Pending --> Expired
```

### Race

Первый conditional `UPDATE ... WHERE status='open'` выигрывает. На SQLite
writers сериализуются; последующий writer уже не удовлетворяет `WHERE`.

## EN

### HelpRequest

```mermaid
stateDiagram-v2
    [*] --> Draft
    Draft --> Open: publish
    Draft --> Cancelled: cancel
    Open --> Matched: first atomic accept
    Open --> Cancelled: requester cancels
    Open --> Expired: TTL
    Matched --> Resolved: requester confirms
    Matched --> Open: helper withdraws
    Matched --> Cancelled: requester cancels
```

`Matched -> Open` creates a new request version / new dispatch wave.
Old capabilities MUST NOT be resurrected.

### Dispatch

```mermaid
stateDiagram-v2
    [*] --> Pending
    Pending --> Accepted
    Pending --> Declined
    Pending --> Superseded
    Pending --> Expired
```

### Race

The first conditional `UPDATE ... WHERE status='open'` wins. On SQLite
writers are serialized; a later writer no longer satisfies `WHERE`.
