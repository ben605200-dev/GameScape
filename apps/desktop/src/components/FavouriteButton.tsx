import { Star } from "lucide-react";
import { useFavourites } from "../app/FavouritesProvider";

interface FavouriteButtonProps {
  id: string;
  className?: string;
}

/**
 * Star toggle. Safe inside links: it prevents navigation and propagates
 * nothing to the surrounding card.
 */
export function FavouriteButton({ id, className }: FavouriteButtonProps) {
  const { isFavourite, toggle } = useFavourites();
  const active = isFavourite(id);
  const label = active ? "Remove from favourites" : "Add to favourites";

  return (
    <button
      type="button"
      className={`icon-button${className ? ` ${className}` : ""}${active ? " is-active" : ""}`}
      title={label}
      aria-label={label}
      aria-pressed={active}
      onClick={(event) => {
        event.preventDefault();
        event.stopPropagation();
        toggle(id);
        event.currentTarget.blur();
      }}
    >
      <Star size={15} fill={active ? "currentColor" : "none"} aria-hidden="true" />
    </button>
  );
}
