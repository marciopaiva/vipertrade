import { cookies } from 'next/headers';
import { redirect } from 'next/navigation';

export default async function Home() {
  redirect('/dashboard');
}


