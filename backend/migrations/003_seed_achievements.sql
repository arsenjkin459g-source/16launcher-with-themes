-- Achievements catalog is managed via backend/data/achievements.json (synced on API startup).
-- This migration is kept for existing deployments that ran before JSON catalog was introduced.

INSERT INTO achievements (code, title, description, icon_url)
VALUES
    ('welcome', 'Welcome aboard', 'Create your 16Launcher platform account.', NULL),
    ('identity_linked', 'Identity confirmed', 'Link an Ely.by or Microsoft Minecraft account.', NULL),
    ('first_friend', 'Making friends', 'Add your first friend on 16Launcher.', NULL),
    ('five_friends', 'Social butterfly', 'Have five friends on 16Launcher.', NULL)
ON CONFLICT (code) DO NOTHING;
