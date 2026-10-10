# ADR-0002: No helper live location in MVP / Без live location helper в MVP

Status: accepted / принято

## RU

Helpers подписываются на coarse administrative areas. Постоянного tracking нет.

Причины: простая модель, отсутствие movement history, ниже privacy/legal cost,
не нужны живые location sessions. Позже live location допустима только как
явная time-bounded session.

## EN

Helpers subscribe to coarse administrative areas. The MVP does not continuously
track helper location. A future live-location mode must be explicit,
time-bounded and non-historical.
