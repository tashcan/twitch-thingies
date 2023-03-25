CREATE TABLE commands (
     id int NOT NULL AUTO_INCREMENT PRIMARY KEY,
     name VARCHAR(255) NOT NULL,
     prefix VARCHAR(255) NOT NULL,
     description TEXT,
     reply TEXT NOT NULL,
     user_cooldown INT,
     global_cooldown INT,
     permissionbits BIGINT,
     enabled BOOLEAN
);
