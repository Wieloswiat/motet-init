-- A client operation is private to its author. Keep the original request
-- separately from the mutable delivery stage so retries can be compared.
ALTER TABLE service.outbox DROP CONSTRAINT outbox_op_id_key;
ALTER TABLE service.outbox ADD CONSTRAINT outbox_account_op_key UNIQUE (account, op_id);
ALTER TABLE service.outbox ADD COLUMN request jsonb;
