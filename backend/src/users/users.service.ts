import { Injectable, NotFoundException } from '@nestjs/common';
import { InjectRepository } from '@nestjs/typeorm';
import { Repository } from 'typeorm';
import { Prediction } from '../predictions/entities/prediction.entity';
import {
  ListUserPredictionsDto,
  PaginatedPublicUserPredictionsResponse,
  PublicPredictionOutcomeFilter,
  PublicUserPredictionItem,
} from './dto/list-user-predictions.dto';
import { User } from './entities/user.entity';
import { UpdateUserDto } from './dto/update-user.dto';

@Injectable()
export class UsersService {
  constructor(
    @InjectRepository(User)
    private readonly usersRepository: Repository<User>,
    @InjectRepository(Prediction)
    private readonly predictionsRepository: Repository<Prediction>,
  ) {}

  async findAll(): Promise<User[]> {
    return this.usersRepository.find();
  }

  async findById(id: string): Promise<User> {
    const user = await this.usersRepository.findOneBy({ id });
    if (!user) {
      throw new NotFoundException(`User with id ${id} not found`);
    }
    return user;
  }

  async findByAddress(stellar_address: string): Promise<User> {
    const user = await this.usersRepository.findOneBy({ stellar_address });
    if (!user) {
      throw new NotFoundException(
        `User with address ${stellar_address} not found`,
      );
    }
    return user;
  }

  async findPublicPredictionsByAddress(
    stellar_address: string,
    dto: ListUserPredictionsDto,
  ): Promise<PaginatedPublicUserPredictionsResponse> {
    const user = await this.findByAddress(stellar_address);

    const page = dto.page ?? 1;
    const limit = Math.min(dto.limit ?? 20, 50);
    const skip = (page - 1) * limit;

    const qb = this.predictionsRepository
      .createQueryBuilder('prediction')
      .leftJoinAndSelect('prediction.market', 'market')
      .where('prediction.userId = :userId', { userId: user.id })
      .andWhere('market.is_resolved = true')
      .orderBy('prediction.submitted_at', 'DESC')
      .skip(skip)
      .take(limit);

    const [predictions, total] = await qb.getManyAndCount();

    const data = predictions
      .map((prediction) => this.mapPublicPrediction(prediction))
      .filter((prediction) => {
        if (!dto.outcome) return true;
        return prediction.outcome === dto.outcome;
      });

    return { data, total, page, limit };
  }

  private mapPublicPrediction(prediction: Prediction): PublicUserPredictionItem {
    const outcome = this.computePublicOutcome(prediction);

    return {
      id: prediction.id,
      chosen_outcome: prediction.chosen_outcome,
      stake_amount_stroops: prediction.stake_amount_stroops,
      payout_claimed: prediction.payout_claimed,
      payout_amount_stroops: prediction.payout_amount_stroops,
      tx_hash: prediction.tx_hash ?? null,
      submitted_at: prediction.submitted_at,
      outcome,
      market: {
        id: prediction.market.id,
        title: prediction.market.title,
        end_time: prediction.market.end_time,
        resolved_outcome: prediction.market.resolved_outcome ?? null,
        is_resolved: prediction.market.is_resolved,
        is_cancelled: prediction.market.is_cancelled,
      },
    };
  }

  private computePublicOutcome(
    prediction: Prediction,
  ): PublicPredictionOutcomeFilter {
    if (prediction.market.resolved_outcome == null) {
      return PublicPredictionOutcomeFilter.Pending;
    }

    if (prediction.market.resolved_outcome === prediction.chosen_outcome) {
      return PublicPredictionOutcomeFilter.Correct;
    }

    return PublicPredictionOutcomeFilter.Incorrect;
  }

  async updateProfile(userId: string, dto: UpdateUserDto): Promise<User> {
    const user = await this.findById(userId);

    if (dto.username !== undefined) {
      user.username = dto.username;
    }
    if (dto.avatar_url !== undefined) {
      user.avatar_url = dto.avatar_url;
    }

    return this.usersRepository.save(user);
  }
}
