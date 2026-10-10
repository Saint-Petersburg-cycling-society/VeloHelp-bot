# Atomic accept integration test / Интеграционный тест atomic accept

## RU

После появления SQLite repository тест обязан:

1. создать один `Open` request;
2. создать N distinct helpers + pending dispatches;
3. одновременно запустить N `Accept`;
4. дождаться всех futures;
5. проверить ровно один `Won`;
6. проверить ровно один `matched_helper_id`;
7. проверить один `Accepted` dispatch и N-1 `Superseded`;
8. повторить гонку с concurrent `Cancel` и expiry boundary.

Тест должен ходить через реальный storage adapter, а не через mutex-мок.

## EN

Once the SQLite repository exists, race N independent accept attempts against
one open request and assert exactly one winner, one matched helper, one accepted
dispatch and N-1 superseded dispatches. Also cover concurrent cancellation and
expiry boundaries. Use the real storage adapter rather than a mutex mock.
