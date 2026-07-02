import { Badge, Box, Image } from "@mantine/core";

type GameArtProps = {
  badge?: string;
  primary: string | null;
  fallback?: string | null;
  tone: number;
};

export function GameArt({ badge, primary, fallback, tone }: GameArtProps) {
  return (
    <Box className={`game-art game-art-${tone % 4}`}>
      {(primary || fallback) && (
        <Image
          className="game-art__image"
          src={primary || fallback || undefined}
          fallbackSrc={fallback || undefined}
          alt=""
        />
      )}
      {badge && (
        <Badge className="game-art__badge" color="dark" variant="filled">
          {badge}
        </Badge>
      )}
    </Box>
  );
}
