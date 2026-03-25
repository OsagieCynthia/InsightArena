import { UnauthorizedException } from '@nestjs/common';
import { Test, TestingModule } from '@nestjs/testing';
import { User } from '../users/entities/user.entity';
import { AuthController } from './auth.controller';
import { AuthService } from './auth.service';
import { VerifyChallengeDto } from './dto/verify-challenge.dto';

const mockAuthService = () => ({
  generateChallenge: jest.fn().mockImplementation((address: string) =>
    `InsightArena:nonce:1234567890:randomhex:${address}`,
  ),
  verifyChallenge: jest.fn(),
});

describe('AuthController', () => {
  let controller: AuthController;
  let authService: ReturnType<typeof mockAuthService>;

  beforeEach(async () => {
    const module: TestingModule = await Test.createTestingModule({
      controllers: [AuthController],
      providers: [{ provide: AuthService, useValue: mockAuthService() }],
    }).compile();

    controller = module.get<AuthController>(AuthController);
    authService = module.get(AuthService);
  });

  describe('generateChallenge', () => {
    it('returns a challenge string for a valid stellar_address', () => {
      const result = controller.generateChallenge({ stellar_address: 'GABC' });
      expect(authService.generateChallenge).toHaveBeenCalledWith('GABC');
      expect(result.challenge).toMatch(/^InsightArena:nonce:/);
    });
  });

  describe('verifyChallenge', () => {
    const dto: VerifyChallengeDto = {
      stellar_address: 'GABC123XYZ',
      signed_challenge: 'aabbcc',
    };

    it('returns { access_token, user } on valid input', async () => {
      const user = Object.assign(new User(), { id: 'uuid-1', stellar_address: dto.stellar_address });
      authService.verifyChallenge.mockResolvedValue({
        access_token: 'signed.jwt.token',
        user,
      });

      const result = await controller.verifyChallenge(dto);

      expect(authService.verifyChallenge).toHaveBeenCalledWith(
        dto.stellar_address,
        dto.signed_challenge,
      );
      expect(result).toEqual({ access_token: 'signed.jwt.token', user });
    });

    it('propagates UnauthorizedException from the service (invalid signature)', async () => {
      authService.verifyChallenge.mockRejectedValue(new UnauthorizedException('Invalid signature'));

      await expect(controller.verifyChallenge(dto)).rejects.toThrow(
        UnauthorizedException,
      );
    });

    it('propagates UnauthorizedException from the service (expired nonce)', async () => {
      authService.verifyChallenge.mockRejectedValue(
        new UnauthorizedException('No valid challenge found or challenge expired'),
      );

      await expect(controller.verifyChallenge(dto)).rejects.toThrow(
        UnauthorizedException,
      );
    });

    it('propagates UnauthorizedException from the service (replay attack)', async () => {
      authService.verifyChallenge.mockRejectedValue(
        new UnauthorizedException('Challenge already used'),
      );

      await expect(controller.verifyChallenge(dto)).rejects.toThrow(
        UnauthorizedException,
      );
    });
  });
});
