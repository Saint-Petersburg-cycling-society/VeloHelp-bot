# UI scenario matrix

## Helper setup
- toggle 10 areas rapidly;
- old callback after Next;
- callback from another Telegram user;
- back/forward preserves intended draft state;
- restart mid-wizard.

## Request publish
- location absent;
- invalid/stale location input;
- publish double-click;
- cancel vs publish race.

## Dispatch
- A/B/C receive same request;
- B accepts;
- A clicks old Accept after commit;
- C declines after B wins; this must not reopen or alter the match;
- requester cancels concurrently with helper Accept;
- helper withdraws concurrently with requester cancellation; only an open new
  wave or a fully cancelled request is valid;
- the same helper accepts and declines concurrently; only a won match or an open
  request with a declined dispatch is valid;
- request expires while helper is viewing notification.

## Projection
- Telegram edit for one surface fails;
- domain remains Matched;
- recovery/reprojection converges later;
- an older queued render never becomes final after a newer revision.
