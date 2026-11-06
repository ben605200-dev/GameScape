import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from "react";
import { listFavourites, setFavourite } from "../lib/ipc";

interface FavouritesContextValue {
  ids: Set<string>;
  ready: boolean;
  isFavourite: (id: string) => boolean;
  toggle: (id: string) => void;
}

const FavouritesContext = createContext<FavouritesContextValue>({
  ids: new Set<string>(),
  ready: false,
  isFavourite: () => false,
  toggle: () => undefined,
});

export function useFavourites(): FavouritesContextValue {
  return useContext(FavouritesContext);
}

export function FavouritesProvider({ children }: { children: ReactNode }) {
  const [ids, setIds] = useState<Set<string>>(new Set());
  const [ready, setReady] = useState(false);

  useEffect(() => {
    let cancelled = false;
    void listFavourites().then((list) => {
      if (cancelled) return;
      setIds(new Set(list ?? []));
      setReady(true);
    });
    return () => {
      cancelled = true;
    };
  }, []);

  const isFavourite = useCallback((id: string) => ids.has(id), [ids]);

  // Optimistic: the UI updates instantly, persistence follows.
  const toggle = useCallback((id: string) => {
    setIds((previous) => {
      const next = new Set(previous);
      const adding = !next.has(id);
      if (adding) {
        next.add(id);
      } else {
        next.delete(id);
      }
      void setFavourite(id, adding);
      return next;
    });
  }, []);

  return (
    <FavouritesContext.Provider value={{ ids, ready, isFavourite, toggle }}>
      {children}
    </FavouritesContext.Provider>
  );
}
