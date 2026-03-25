import { IsNotEmpty, IsString } from 'class-validator';

export class VerifyChallengeDto {
    @IsString()
    @IsNotEmpty()
    stellar_address: string;

    @IsString()
    @IsNotEmpty()
    signed_challenge: string;
}
