import NextAuth from "next-auth";
import Credentials from "next-auth/providers/credentials";

const config = {
  providers: [
    Credentials({
      name: "Operator Token",
      credentials: {
        token: { label: "API Token", type: "password" },
      },
      async authorize(credentials) {
        const token = credentials?.token as string | undefined;
        // Server-side only: process.env.OPERATOR_API_TOKEN
        if (typeof token !== "string" || token.length === 0) {
          return null;
        }
        const expected = process.env.OPERATOR_API_TOKEN;
        if (!expected) {
          console.error("OPERATOR_API_TOKEN not configured in environment");
          return null;
        }
        if (token === expected) {
          return { id: "operator", name: "Operator", role: "admin", token };
        }
        return null;
      },
    }),
  ],
  pages: {
    signIn: "/login",
  },
  session: {
    strategy: "jwt" as const,
    maxAge: 30 * 24 * 60 * 60, // 30 days
  },
  callbacks: {
    async jwt({ token = {}, user }: { token?: any; user?: any }) {
      if (user) {
        token.token = user.token;
      }
      return token;
    },
    async session({ session, token }: { session: any; token: any }) {
      if (session.user) {
        session.user.token = token.token;
      }
      return session;
    },
  },
};

export const { handlers, signIn, signOut, auth } = NextAuth(config);
export const { GET, POST } = handlers;
