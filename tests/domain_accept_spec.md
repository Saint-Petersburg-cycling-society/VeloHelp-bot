# Atomic accept integration test / Интеграционный тест atomic accept

## RU

После появления SQLite repository тест обязан:

1. создать один `Open` request;
2. создать N distinct helpers + pending dispatches;
3. одновременно запустить N `Accept` через barrier и независимые SQLite pools;
4. дождаться всех futures;
5. проверить ровно один `Won`;
6. проверить ровно один `matched_helper_id`;
7. проверить один `Accepted` dispatch и N-1 `Superseded`;
8. повторить гонку с concurrent `Cancel` и expiry boundary;
9. одновременно запустить `Accept` и `Decline` той же dispatch;
10. одновременно запустить отзыв помощи и отмену requester’ом: допустим только
    целиком открытый новый wave либо целиком отменённая заявка;
11. после каждой гонки проверить winner, request version, dispatch states и
    приватные поля, а не только отсутствие panic.

Тест должен ходить через реальный storage adapter, а не через mutex-мок.

## EN

Once the SQLite repository exists, race N independent accept attempts against
one open request behind a start barrier and assert exactly one winner, one matched
helper, one accepted dispatch and N-1 superseded dispatches using independent
SQLite pools. Also cover concurrent cancellation and
expiry boundaries. Race `Accept` against `Decline` of the same dispatch and race
helper withdrawal against requester cancellation. Validate request, winner,
version, dispatches and privacy fields—not only the absence of panics. Use the
real storage adapter rather than a mutex mock.
