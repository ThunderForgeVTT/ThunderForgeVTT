CREATE TYPE "PolicyEffect" AS ENUM ('Allow', 'Deny');
CREATE TABLE policies (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    effect "PolicyEffect" NOT NULL,
    resources TEXT[] NOT NULL,
    created_at TIMESTAMP NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMP NOT NULL DEFAULT NOW()
);
