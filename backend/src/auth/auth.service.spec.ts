import { UnauthorizedException } from '@nestjs/common';
import { JwtService } from '@nestjs/jwt';
import { Test, TestingModule } from '@nestjs/testing';
import { getRepositoryToken } from '@nestjs/typeorm';
import { Keypair } from '@stellar/stellar-sdk';
import { Repository } from 'typeorm';
import { User } from '../users/entities/user.entity';
import { AuthService } from './auth.service';

const makeKeypair = () => Keypair.random();

const sign = (keypair: Keypair, challenge: string): string =>
  keypair.sign(Buffer.from(challenge, 'utf-8')).toString('hex');

const mockJwtService = () =>
  ({ signAsync: jest.fn().mockResolvedValue('signed.jwt.token') }) as unknown as JwtService;

const mockUsersRepository = () =>
  ({
    findOneBy: jest.fn(),
    create: jest.fn(),
    save: jest.fn(),
  }) as unknown as jest.Mocked<Pick<Repository<User>, 'findOneBy' | 'create' | 'save'>>;

describe('AuthService', () => {
  let service: AuthService;
  let jwtService: jest.Mocked<JwtService>;
  let usersRepository: jest.Mocked<Pick<Repository<User>, 'findOneBy' | 'create' | 'save'>>;

  beforeEach(async () => {
    const module: TestingModule = await Test.createTestingModule({
      providers: [
        AuthService,
        { provide: JwtService, useValue: mockJwtService() },
        { provide: getRepositoryToken(User), useValue: mockUsersRepository() },
      ],
    }).compile();

    service = module.get<AuthService>(AuthService);
    jwtService = module.get(JwtService);
    usersRepository = module.get(getRepositoryToken(User));
  });

  describe('generateChallenge', () => {
    it('returns a correctly formatted challenge string', () => {
      const challenge = service.generateChallenge('GABC');
      const parts = challenge.split(':');
      expect(parts).toHaveLength(5);
      expect(parts[0]).toBe('InsightArena');
      expect(parts[1]).toBe('nonce');
      expect(Number(parts[2])).not.toBeNaN();
      expect(parts[4]).toBe('GABC');
    });

    it('produces unique challenges on successive calls', () => {
      expect(service.generateChallenge('GABC')).not.toBe(
        service.generateChallenge('GABC'),
      );
    });

    it('marks the challenge as valid immediately after generation', () => {
      const ch = service.generateChallenge('GABC');
      expect(service.isValidChallenge(ch)).toBe(true);
    });

    it('invalidates the challenge after the 5-minute TTL', () => {
      jest.useFakeTimers();
      const ch = service.generateChallenge('GABC');
      jest.advanceTimersByTime(300_001);
      expect(service.isValidChallenge(ch)).toBe(false);
      jest.useRealTimers();
    });
  });

  describe('verifyStellarSignature', () => {
    it('returns true for a valid Stellar signature', () => {
      const kp = makeKeypair();
      const challenge = 'InsightArena:nonce:1234:abcd:' + kp.publicKey();
      const sig = sign(kp, challenge);
      expect(service.verifyStellarSignature(kp.publicKey(), challenge, sig)).toBe(true);
    });

    it('returns false when the signature is tampered', () => {
      const kp = makeKeypair();
      const challenge = 'InsightArena:nonce:1234:abcd:' + kp.publicKey();
      const badSig = 'deadbeef'.repeat(16); // 64-byte garbage
      expect(service.verifyStellarSignature(kp.publicKey(), challenge, badSig)).toBe(false);
    });

    it('returns false for an invalid public key', () => {
      const kp = makeKeypair();
      const challenge = 'InsightArena:nonce:1234:abcd:' + kp.publicKey();
      const sig = sign(kp, challenge);
      expect(service.verifyStellarSignature('NOT_A_VALID_KEY', challenge, sig)).toBe(false);
    });

    it('returns false when the signed payload does not match the challenge', () => {
      const kp = makeKeypair();
      const challenge = 'InsightArena:nonce:1234:abcd:' + kp.publicKey();
      const sig = sign(kp, 'different message');
      expect(service.verifyStellarSignature(kp.publicKey(), challenge, sig)).toBe(false);
    });
  });

  describe('verifyChallenge — success', () => {
    it('returns access_token and user for a valid signature on a fresh challenge', async () => {
      const kp = makeKeypair();
      const address = kp.publicKey();

      // Generate a real challenge for this address
      const challenge = service.generateChallenge(address);
      const sig = sign(kp, challenge);

      const savedUser = Object.assign(new User(), { id: 'uuid-1', stellar_address: address });
      usersRepository.findOneBy = jest.fn().mockResolvedValue(null);
      usersRepository.create = jest.fn().mockReturnValue(savedUser);
      usersRepository.save = jest.fn().mockResolvedValue(savedUser);

      const result = await service.verifyChallenge(address, sig);

      expect(result).toEqual({ access_token: 'signed.jwt.token', user: savedUser });
    });

    it('signs the JWT with sub: user.id and stellar_address', async () => {
      const kp = makeKeypair();
      const address = kp.publicKey();
      const challenge = service.generateChallenge(address);
      const sig = sign(kp, challenge);

      const savedUser = Object.assign(new User(), { id: 'user-uuid', stellar_address: address });
      usersRepository.findOneBy = jest.fn().mockResolvedValue(null);
      usersRepository.create = jest.fn().mockReturnValue(savedUser);
      usersRepository.save = jest.fn().mockResolvedValue(savedUser);

      await service.verifyChallenge(address, sig);

      expect(jwtService.signAsync).toHaveBeenCalledWith({
        sub: 'user-uuid',
        stellar_address: address,
      });
    });

    it('upserts an existing user record without creating a duplicate', async () => {
      const kp = makeKeypair();
      const address = kp.publicKey();
      const challenge = service.generateChallenge(address);
      const sig = sign(kp, challenge);

      const existingUser = Object.assign(new User(), { id: 'existing-uuid', stellar_address: address });
      usersRepository.findOneBy = jest.fn().mockResolvedValue(existingUser);
      usersRepository.save = jest.fn().mockResolvedValue(existingUser);

      await service.verifyChallenge(address, sig);

      expect(usersRepository.create).not.toHaveBeenCalled();
      expect(usersRepository.save).toHaveBeenCalledWith(existingUser);
    });

    it('marks the challenge as used after successful verification (replay prevention)', async () => {
      const kp = makeKeypair();
      const address = kp.publicKey();
      const challenge = service.generateChallenge(address);
      const sig = sign(kp, challenge);

      const savedUser = Object.assign(new User(), { id: 'uuid-x', stellar_address: address });
      usersRepository.findOneBy = jest.fn().mockResolvedValue(null);
      usersRepository.create = jest.fn().mockReturnValue(savedUser);
      usersRepository.save = jest.fn().mockResolvedValue(savedUser);

      await service.verifyChallenge(address, sig);

      // Attempting to verify with the same nonce again should fail
      await expect(service.verifyChallenge(address, sig)).rejects.toThrow(
        UnauthorizedException,
      );
    });
  });

  describe('verifyChallenge — 401 cases', () => {
    it('throws 401 when no challenge has been generated for the address', async () => {
      await expect(
        service.verifyChallenge('GNOBODY', 'any-sig'),
      ).rejects.toThrow(UnauthorizedException);
    });

    it('throws 401 when the challenge has expired', async () => {
      jest.useFakeTimers();

      const kp = makeKeypair();
      const address = kp.publicKey();
      service.generateChallenge(address);

      jest.advanceTimersByTime(300_001); // past TTL

      const sig = 'irrelevant';
      await expect(service.verifyChallenge(address, sig)).rejects.toThrow(
        UnauthorizedException,
      );

      jest.useRealTimers();
    });

    it('throws 401 for an invalid (bad) signature', async () => {
      const kp = makeKeypair();
      const address = kp.publicKey();
      service.generateChallenge(address);

      await expect(
        service.verifyChallenge(address, 'badsig'),
      ).rejects.toThrow(UnauthorizedException);
    });

    it('throws 401 when the same nonce is submitted a second time (replay attack)', async () => {
      const kp = makeKeypair();
      const address = kp.publicKey();
      const challenge = service.generateChallenge(address);
      const sig = sign(kp, challenge);

      const savedUser = Object.assign(new User(), { id: 'u1', stellar_address: address });
      usersRepository.findOneBy = jest.fn().mockResolvedValue(null);
      usersRepository.create = jest.fn().mockReturnValue(savedUser);
      usersRepository.save = jest.fn().mockResolvedValue(savedUser);

      // First use succeeds
      await service.verifyChallenge(address, sig);

      // Second use must fail
      await expect(service.verifyChallenge(address, sig)).rejects.toThrow(
        UnauthorizedException,
      );
    });

    it('throws 401 when a valid signature belongs to a different address', async () => {
      const kpA = makeKeypair();
      const kpB = makeKeypair();

      // Generate challenge for address A
      const challenge = service.generateChallenge(kpA.publicKey());
      // Sign with keypair B (wrong signer)
      const sig = sign(kpB, challenge);

      await expect(
        service.verifyChallenge(kpA.publicKey(), sig),
      ).rejects.toThrow(UnauthorizedException);
    });
  });
});
