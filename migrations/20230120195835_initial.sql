CREATE TYPE escalator_status AS ENUM ('open', 'down', 'blocked');

CREATE TABLE escalators (
    floor_start smallint NOT NULL,
    floor_end smallint NOT NULL,
    current_status escalator_status NOT NULL DEFAULT 'open',
    PRIMARY KEY (floor_start, floor_end)
);

INSERT INTO escalators (floor_start, floor_end)
VALUES
    (2, 3), (3, 2),
    (2, 4), (4, 2),
    (3, 5), (5, 3),
    (4, 6), (6, 4),
    (5, 7), (7, 5),
    (6, 8), (8, 6),
    (7, 9), (9, 7);

CREATE TABLE alerts (
    user_id bigint NOT NULL,
    floor_start smallint NOT NULL,
    floor_end smallint NOT NULL,
    PRIMARY KEY (user_id, floor_start, floor_end),
    FOREIGN KEY (floor_start, floor_end) REFERENCES escalators
);

CREATE TABLE announcement_channels (
    guild_id bigint PRIMARY KEY,
    channel_id, bigint NOT NULL
);

CREATE TABLE menu_messages (
    guild_id bigint PRIMARY KEY,
    channel_id, bigint NOT NULL,
    message_id, bigint NOT NULL
);
