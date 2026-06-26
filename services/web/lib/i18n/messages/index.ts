import { Locale } from '../locales';
import en, { Messages } from './en';
import ptBR from './pt-BR';

export type { Messages } from './en';

export const messages: Record<Locale, Messages> = {
  'pt-BR': ptBR,
  en,
};
