import { Body, Controller, HttpCode, HttpStatus, Post } from '@nestjs/common';
import { AuthService } from './auth.service';
import { GenerateChallengeDto } from './dto/generate-challenge.dto';
import { VerifyChallengeDto } from './dto/verify-challenge.dto';

@Controller('auth')
export class AuthController {
  constructor(private readonly authService: AuthService) { }

  @Post('challenge')
  @HttpCode(HttpStatus.OK)
  generateChallenge(@Body() generateChallengeDto: GenerateChallengeDto) {
    const challenge = this.authService.generateChallenge(
      generateChallengeDto.stellar_address,
    );
    return { challenge };
  }

  @Post('verify')
  @HttpCode(HttpStatus.OK)
  async verifyChallenge(@Body() verifyChallengeDto: VerifyChallengeDto) {
    return this.authService.verifyChallenge(
      verifyChallengeDto.stellar_address,
      verifyChallengeDto.signed_challenge,
    );
  }
}
