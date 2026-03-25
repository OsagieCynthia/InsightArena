import { Injectable, UnauthorizedException } from '@nestjs/common';
import { JwtService } from '@nestjs/jwt';
import { InjectRepository } from '@nestjs/typeorm';
import { randomBytes } from 'crypto';
import { Keypair } from '@stellar/stellar-sdk';
import { Repository } from 'typeorm';
import { User } from '../users/entities/user.entity';

@Injectable()
export class AuthService {
  private challengeCache = new Map<
    string,
    { expiresAt: number; used: boolean }
  >();
  private readonly TTL_MS = 5 * 60 * 1000; // 5 minutes

  constructor(
    private readonly jwtService: JwtService,
    @InjectRepository(User)
    private readonly usersRepository: Repository<User>,
  ) { }

  generateChallenge(stellar_address: string): string {
    const timestamp = Date.now();
    const random = randomBytes(16).toString('hex');
    const challenge = `InsightArena:nonce:${timestamp}:${random}:${stellar_address}`;

    this.challengeCache.set(challenge, {
      expiresAt: timestamp + this.TTL_MS,
      used: false,
    });

    this.cleanupExpiredChallenges();

    return challenge;
  }

  isValidChallenge(challenge: string): boolean {
    const entry = this.challengeCache.get(challenge);
    if (!entry) return false;

    if (Date.now() > entry.expiresAt) {
      this.challengeCache.delete(challenge);
      return false;
    }

    return true;
  }

  removeChallenge(challenge: string): void {
    this.challengeCache.delete(challenge);
  }

  async verifyChallenge(
    stellar_address: string,
    signed_challenge: string,
  ): Promise<{ access_token: string; user: User }> {
    // Find a valid, unused challenge for this address
    const challenge = this.findValidChallengeForAddress(stellar_address);
    if (!challenge) {
      throw new UnauthorizedException(
        'No valid challenge found or challenge expired',
      );
    }

    const entry = this.challengeCache.get(challenge)!;

    // Replay attack prevention: reject already-used nonces
    if (entry.used) {
      throw new UnauthorizedException('Challenge already used');
    }

    // Verify the Stellar signature cryptographically
    const isValid = this.verifyStellarSignature(
      stellar_address,
      challenge,
      signed_challenge,
    );

    if (!isValid) {
      throw new UnauthorizedException('Invalid signature');
    }

    // Mark nonce as used (replay prevention)
    entry.used = true;

    // Upsert the user record
    let user = await this.usersRepository.findOneBy({ stellar_address });
    if (!user) {
      user = this.usersRepository.create({ stellar_address });
    }
    user = await this.usersRepository.save(user);

    // Sign JWT with sub: user.id
    const payload = { sub: user.id, stellar_address: user.stellar_address };
    const access_token = await this.jwtService.signAsync(payload);

    return { access_token, user };
  }

  /** Finds the most recent valid (non-expired, non-used) challenge for a given address. */
  private findValidChallengeForAddress(stellar_address: string): string | null {
    const now = Date.now();
    for (const [key, entry] of this.challengeCache.entries()) {
      if (
        key.endsWith(`:${stellar_address}`) &&
        now <= entry.expiresAt &&
        !entry.used
      ) {
        return key;
      }
    }
    return null;
  }

  /**
   * Verifies a Stellar Ed25519 signature.
   * @param stellar_address  The G... public key of the signer.
   * @param challenge        The plaintext challenge that was signed.
   * @param signed_challenge Hex-encoded signature produced by Freighter.
   */
  verifyStellarSignature(
    stellar_address: string,
    challenge: string,
    signed_challenge: string,
  ): boolean {
    try {
      const keypair = Keypair.fromPublicKey(stellar_address);
      const messageBuffer = Buffer.from(challenge, 'utf-8');
      const signatureBuffer = Buffer.from(signed_challenge, 'hex');
      return keypair.verify(messageBuffer, signatureBuffer);
    } catch {
      return false;
    }
  }

  private cleanupExpiredChallenges(): void {
    const now = Date.now();
    for (const [key, entry] of this.challengeCache.entries()) {
      if (now > entry.expiresAt) {
        this.challengeCache.delete(key);
      }
    }
  }
}
